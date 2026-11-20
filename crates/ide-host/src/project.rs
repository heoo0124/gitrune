//! One open project folder: documents, repositories, watchers and local history,
//! opened by [`crate::Host::open_project`].

use crate::Host;
use ide_core::{IdeError, IdeEvent, IdeResult, LanguageId, SharedSink};
use ide_vcs::{Repo, Repos};
use ide_vfs::Watcher;
use ide_workspace::documents::DocumentObserver;
use ide_workspace::{Documents, LocalHistory, SettingsStore, SourceFolder};
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Everything bound to one open project folder.
pub struct Project {
    pub(crate) trusted: Arc<AtomicBool>,
    pub(crate) files_events_active: Arc<AtomicBool>,
    pub root: PathBuf,
    /// Window-scoped event sink, for commands that emit project events.
    pub sink: SharedSink,
    pub settings: Arc<SettingsStore>,
    pub documents: Arc<Documents>,
    pub history: Arc<LocalHistory>,
    /// Git repositories: the one containing the project root and those nested below it.
    pub repos: Arc<RwLock<Repos>>,
    /// Explicitly opened external logs remain read-only and outside project indexing.
    pub(crate) external_log_repos: RwLock<Vec<Arc<Repo>>>,
    /// Per-project cache directory (build tool sync results…).
    pub cache_dir: PathBuf,
    pub(crate) watcher: Arc<Watcher>,
    /// A LightEdit window's loose files ([`Host::open_light`]): global settings only,
    /// nothing indexed, no language servers, repositories or project services.
    pub light: bool,
}

impl Host {
    pub fn open_project(&self, window: &str, root: &Path) -> IdeResult<Arc<Project>> {
        let started = std::time::Instant::now();
        let root = dunce::canonicalize(root)?;
        let opening = self.opening(window);
        let _opening = opening.lock();
        // Reject a damaged manifest before replacing the currently open project.
        ide_workspace::workspace::Workspace::load(&root)?;
        // The window reloaded (or asked again) for the project it already has: keep
        // it, with its warm repositories and unsaved documents.
        if let Ok(current) = self.project(window)
            && current.root == root
            && !current.light
        {
            return Ok(current);
        }
        self.close_project(window);
        self.wait_detached_closes();
        let sink: SharedSink = (self.sink_for)(window);
        // Before anything the project starts: its children get the project's environment.
        let trusted = Arc::new(AtomicBool::new(self.trust.trusted(&root)?));
        let post_start =
            trusted.load(Ordering::Acquire).then(|| self.environment.project_opened(&root, &sink)).flatten();
        let settings = SettingsStore::load(&self.config_dir, Some(&root));
        let detected_roots = Arc::new(RwLock::new(ide_workspace::roots::detect(&root)));
        let excludes = exclude_globs(&root, &settings.effective(), &detected_roots.read());
        let project_key = project_key(&root);

        let files_events_active = Arc::new(AtomicBool::new(true));
        let documents = documents(&sink, Some(&root));
        documents.set_file_defaults(ide_workspace::FileDefaults::from_settings(&settings.effective(), Some(&root)));
        let history = Arc::new(LocalHistory::open(&self.data_dir.join("history"), &root)?);
        {
            let history = history.clone();
            let retention = history_retention(&settings.effective());
            std::thread::Builder::new().name("gitrune-history-prune".into()).spawn(move || {
                if let Err(e) = history.apply_retention(retention) {
                    tracing::warn!("local history: {e}");
                }
            })?;
        }
        let settings = Arc::new(settings);
        documents.add_observer(Arc::new(HistoryObserver(history.clone())));

        // Git repositories: the one containing the root and those nested below it.
        let repos = Arc::new(RwLock::new(Repos::default()));
        let watcher = Arc::new(project_root_watcher(
            &root,
            &excludes,
            &sink,
            &documents,
            &history,
            files_events_active.clone(),
            {
                let (root, repos, settings, sink) = (root.clone(), repos.clone(), settings.clone(), sink.clone());
                let cache = self.local_data_dir.join("cache");
                move || scan_repositories(&root, &repos, &settings, &sink, &cache)
            },
        )?);
        {
            let (root, repos, settings, sink) = (root.clone(), repos.clone(), settings.clone(), sink.clone());
            let cache = self.local_data_dir.join("cache");
            std::thread::Builder::new()
                .name("gitrune-vcs-roots".into())
                .spawn(move || scan_repositories(&root, &repos, &settings, &sink, &cache))?;
        }
        tracing::info!("opened {} in window {window} in {} ms", root.display(), started.elapsed().as_millis());

        let project = Arc::new(Project {
            trusted,
            root,
            sink,
            settings,
            documents,
            history,
            repos,
            external_log_repos: RwLock::default(),
            cache_dir: self.local_data_dir.join("cache").join(&project_key),
            watcher,
            files_events_active,
            light: false,
        });
        self.insert_project(window, project.clone());
        if let Some(cmd) = post_start {
            self.environment.spawn_post_start(cmd, project.root.clone(), project.sink.clone());
        }
        Ok(project)
    }
}

impl Host {
    /// Opens `window` for editing loose files (LightEdit), `file` first: documents
    /// with syntax highlighting, saving and Local History over the global settings,
    /// without an index, language servers, repositories or project settings. A
    /// window already in LightEdit keeps its session; only its watchers grow.
    pub fn open_light(&self, window: &str, file: &Path) -> IdeResult<Arc<Project>> {
        let file = dunce::canonicalize(file)?;
        if file.is_dir() {
            return Err(IdeError::invalid(format!("{} is a folder", file.display())));
        }
        let dir = file.parent().map(Path::to_path_buf).ok_or_else(|| IdeError::invalid("no parent folder"))?;
        let opening = self.opening(window);
        let _opening = opening.lock();
        if let Ok(current) = self.project(window)
            && current.light
        {
            current.watch_light_dir(&dir);
            return Ok(current);
        }
        self.close_project(window);
        self.wait_detached_closes();
        let sink: SharedSink = (self.sink_for)(window);
        let settings = Arc::new(SettingsStore::load(&self.config_dir, None));
        let effective = settings.effective();
        let documents = documents(&sink, None);
        documents.set_file_defaults(ide_workspace::FileDefaults::from_settings(&effective, None));
        let history = Arc::new(LocalHistory::open(&self.data_dir.join("history"), &dir)?);
        documents.add_observer(Arc::new(HistoryObserver(history.clone())));
        let watcher = light_watcher(&dir, &sink, &documents, &history)?;
        let project_key = project_key(&dir);
        let project = Arc::new(Project {
            trusted: Arc::new(AtomicBool::new(false)),
            sink,
            settings,
            documents,
            history,
            repos: Arc::default(),
            external_log_repos: RwLock::default(),
            cache_dir: self.local_data_dir.join("cache").join(&project_key),
            watcher: Arc::new(watcher),
            files_events_active: Arc::new(AtomicBool::new(false)),
            light: true,
            root: dir,
        });
        tracing::info!("window {window} edits {} without a project", file.display());
        self.insert_project(window, project.clone());
        Ok(project)
    }
}

impl Project {
    pub fn is_trusted(&self) -> bool {
        self.trusted.load(Ordering::Acquire)
    }

    pub fn require_trust(&self) -> IdeResult<()> {
        if self.is_trusted() {
            Ok(())
        } else {
            Err(IdeError::unavailable("Project is in Restricted Mode. Trust the project to enable execution."))
        }
    }

    /// LightEdit: follow external changes to the files of `dir` (its own entries only).
    pub fn watch_light_dir(&self, dir: &Path) {
        if !self.light {
            return;
        }
        let dir = dunce::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        if dir == self.root {
            return;
        }
        // LightEdit keeps one watcher per directory for the lifetime of the window.
        if let Err(e) = light_watcher(&dir, &self.sink, &self.documents, &self.history) {
            tracing::warn!("watching {}: {e}", dir.display());
        }
    }
}

/// The documents of a project whose languages and UI modes `languages` detects,
/// looking for frameworks' root markers up to `root` (anywhere for loose files).
fn documents(sink: &SharedSink, root: Option<&Path>) -> Arc<Documents> {
    // Without a language registry, detect by extension only: enough for the editor's
    // syntax highlighting and for Git's encoding preview.
    let root = root.map(Path::to_path_buf);
    Arc::new(
        Documents::new(
            sink.clone(),
            Box::new(|p: &Path, _first: Option<&str>| {
                LanguageId(p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase())
            }),
        )
        .with_ui_mode_detection(Box::new(move |_p: &Path, _lang: &LanguageId| {
            let _ = &root;
            None
        })),
    )
}

fn light_watcher(
    dir: &Path,
    sink: &SharedSink,
    documents: &Arc<Documents>,
    history: &Arc<LocalHistory>,
) -> IdeResult<Watcher> {
    let (documents, history) = (documents.clone(), history.clone());
    Watcher::start_with(
        dir,
        Vec::new(),
        sink.clone(),
        Box::new(move |batch| {
            let changed: Vec<PathBuf> = batch.created.iter().chain(&batch.modified).cloned().collect();
            documents.on_disk_changed(&changed);
            record_external_changes(&history, &changed, &batch.removed);
        }),
        ide_vfs::WatchMode::Flat,
    )
}

/// Looks for repositories nested below the project (and the one containing it);
/// emits `VcsChanged` when the set of roots changed.
fn scan_repositories(
    root: &Path,
    repos: &RwLock<Repos>,
    settings: &SettingsStore,
    sink: &SharedSink,
    build_cache: &Path,
) {
    let ignored = settings.effective().ignored_names();
    let skip = |p: &Path| {
        p.file_name().is_some_and(|n| {
            let name = n.to_string_lossy();
            ignored.is_ignored_name(&name)
                || (ide_workspace::unity::GENERATED_DIRS.contains(&name.as_ref())
                    && p.parent().is_some_and(ide_workspace::unity::is_project))
        })
    };
    let mut roots = vec![root.to_path_buf()];
    let _ = build_cache;
    roots.extend(content_roots(&settings.effective(), root));
    match Repos::discover_roots(&roots, &skip) {
        Ok(found) => {
            if found.roots() != repos.read().roots() {
                *repos.write() = found;
                sink.emit(IdeEvent::VcsChanged { root: root.to_path_buf().into() });
            }
        }
        Err(e) => tracing::warn!("looking for git repositories: {e}"),
    }
}

impl Project {
    pub fn refresh_repositories(&self) {
        scan_repositories(&self.root, &self.repos, &self.settings, &self.sink, &self.cache_dir);
    }

    /// The project's git repositories.
    pub fn repos(&self) -> Repos {
        let repos = self.repos.read().clone();
        for repo in repos.all() {
            repo.set_execution_allowed(self.is_trusted());
        }
        repos
    }

    /// Runs `f` with the repository whose root is `at`, the project's primary repository
    /// when `None`; fails when there is none (a repository-scoped command never falls back
    /// to another repository).
    pub fn with_repo_at<T>(&self, at: Option<&Path>, f: impl FnOnce(&Repo) -> IdeResult<T>) -> IdeResult<T> {
        let repos = self.repos();
        match at {
            Some(root) => match repos.at_root(root) {
                Some(repo) => self.run_in(repo, f),
                None => {
                    Err(IdeError::unavailable(format!("{} is not a git repository of the project", root.display())))
                }
            },
            None => match repos.primary() {
                Some(repo) => self.run_in(repo, f),
                None => Err(IdeError::unavailable("project is not a git repository")),
            },
        }
    }

    pub fn open_external_log(&self, path: &Path) -> IdeResult<PathBuf> {
        let root = dunce::canonicalize(path)?;
        let repo = Repo::discover(&root)?.ok_or_else(|| IdeError::unavailable("folder is not a Git worktree"))?;
        let root = repo.root().to_path_buf();
        repo.set_execution_allowed(false);
        let mut external = self.external_log_repos.write();
        if !external.iter().any(|r| r.root() == root) {
            external.push(Arc::new(repo));
        }
        Ok(root)
    }

    /// Log read operations may also use an explicitly opened external worktree.
    /// Mutations continue to use `with_repo_at` and cannot reach these repositories.
    pub fn with_log_repo_at<T>(&self, at: Option<&Path>, f: impl FnOnce(&Repo) -> IdeResult<T>) -> IdeResult<T> {
        if let Some(path) = at
            && self.repos().at_root(path).is_none()
        {
            let repo = self
                .external_log_repos
                .read()
                .iter()
                .find(|r| r.root() == path)
                .cloned()
                .ok_or_else(|| IdeError::unavailable("external Git log is not open"))?;
            return f(&repo);
        }
        self.with_repo_at(at, f)
    }

    pub fn with_log_repo_of<T>(&self, path: &Path, f: impl FnOnce(&Repo) -> IdeResult<T>) -> IdeResult<T> {
        if self.repos().for_path(path).is_some() {
            return self.with_repo_of(path, f);
        }
        let repo = self
            .external_log_repos
            .read()
            .iter()
            .filter(|r| path.starts_with(r.root()))
            .max_by_key(|r| r.root().components().count())
            .cloned()
            .ok_or_else(|| IdeError::unavailable("path is not in an open Git log"))?;
        f(&repo)
    }

    /// Runs `f` with the repository `path` (a file or folder) belongs to.
    pub fn with_repo_of<T>(&self, path: &Path, f: impl FnOnce(&Repo) -> IdeResult<T>) -> IdeResult<T> {
        match self.repos().for_path(path) {
            Some(repo) => self.run_in(repo, f),
            None => Err(IdeError::unavailable(format!("{} is not in a git repository", path.display()))),
        }
    }

    fn run_in<T>(&self, repo: &Repo, f: impl FnOnce(&Repo) -> IdeResult<T>) -> IdeResult<T> {
        let settings = self.settings.effective();
        if self.is_trusted() {
            apply_git_executable(&settings);
        }
        repo.set_ignored_names(settings.ignored_names());
        f(repo)
    }

    /// Runs `f` with the project's primary git repository, or fails when the project is not under git.
    pub fn with_repo<T>(&self, f: impl FnOnce(&Repo) -> IdeResult<T>) -> IdeResult<T> {
        self.with_repo_at(None, f)
    }

    pub(crate) fn with_disk_sync<T>(&self, f: impl FnOnce() -> IdeResult<T>) -> IdeResult<T> {
        let result = f();
        // Failed operations can also leave changed files (for example merge conflicts).
        self.documents.sync_after_disk_change();
        result
    }

    /// Runs a VCS operation that rewrites the working tree (checkout, merge,
    /// rebase, pull, reset, stash, rollback): Local History gets a label and a
    /// snapshot of every locally changed file before, and a label after.
    /// Clean buffers follow the resulting files before returning; `None` omits the history labels.
    pub fn with_repo_recorded<T>(
        &self,
        at: Option<&Path>,
        operation: Option<&str>,
        f: impl FnOnce(&Repo) -> IdeResult<T>,
    ) -> IdeResult<T> {
        let Some(name) = operation else { return self.with_disk_sync(|| self.with_repo_at(at, f)) };
        self.history_label(&format!("Before {name}"));
        if let Ok(changed) = self.with_repo_at(at, |r| r.status()) {
            for file in changed.iter().take(HISTORY_BATCH_LIMIT) {
                if let Err(e) = self.history.record_file(&file.path, &format!("Before {name}"), false) {
                    tracing::debug!("local history: {e}");
                }
            }
        }
        let result = self.with_disk_sync(|| self.with_repo_at(at, f));
        self.history_label(&format!("After {name}"));
        result
    }

    fn history_label(&self, label: &str) {
        if let Err(e) = self.history.put_label(&self.root, label) {
            tracing::warn!("local history: {e}");
        }
    }

    /// Local History snapshots of `paths` before a refactoring changes them.
    pub fn record_before_refactoring(&self, paths: &[&Path], name: &str) {
        let label = format!("Before refactoring: {name}");
        for path in paths {
            let result = match self.documents.text_of(path) {
                Some(text) => self.history.record_event(path, &text, &label),
                None => self.history.record_file(path, &label, true).map(|_| ()),
            };
            if let Err(e) = result {
                tracing::debug!("local history: {e}");
            }
        }
    }

    /// `files.exclude`, the ignored names and the effective excluded roots (user-marked and detected).
    pub fn exclude_globs(&self) -> Vec<String> {
        exclude_globs(&self.root, &self.settings.effective(), &[])
    }

    /// Use the open document's selected encoding, or the same detection/settings as opening it.
    pub fn vcs_encoding(&self, path: &Path, rev: Option<&str>) -> IdeResult<ide_vcs::TextEncoding> {
        let charset = self.file_encoding(path, rev)?;
        let encoding = ide_vcs::TextEncoding::from(charset);
        Ok(if self.documents.native_to_ascii_for(path) {
            encoding.with_ascii_conversion(ide_vfs::decode_properties_escapes, ide_vfs::encode_properties_ascii)
        } else {
            encoding
        })
    }

    fn file_encoding(&self, path: &Path, rev: Option<&str>) -> IdeResult<&'static encoding_rs::Encoding> {
        if let Some(id) = self.documents.by_path(path) {
            let info = self.documents.snapshot(id)?;
            return encoding_rs::Encoding::for_label(info.encoding.as_bytes())
                .ok_or_else(|| ide_core::IdeError::invalid("unknown document encoding"));
        }
        let defaults = self.documents.file_defaults();
        let fallback = defaults.fallback_for(path);
        if path.exists() {
            let loaded = ide_vfs::load_text_with_fallback(path, fallback)?;
            return encoding_rs::Encoding::for_label(loaded.encoding.as_bytes())
                .ok_or_else(|| ide_core::IdeError::invalid("unknown file encoding"));
        }
        let bytes = self.with_log_repo_of(path, |r| r.file_bytes_at(path, rev))?.unwrap_or_default();
        Ok(ide_vfs::detect_encoding(&bytes, fallback))
    }

    /// Current text of `path`: the open buffer when available, else the file on disk.
    pub fn text_of(&self, path: &Path) -> IdeResult<String> {
        match self.documents.text_of(path) {
            Some(text) => Ok(text),
            None => Ok(ide_text::normalize_line_endings(&self.documents.load_file(path)?.text).into_owned()),
        }
    }
}

/// Files the watcher reports per batch that Local History snapshots.
const HISTORY_BATCH_LIMIT: usize = 500;

/// Local History for changes made outside the IDE (other tools, VCS, deletions).
/// Contents equal to the last snapshot (e.g. the IDE's own saves) are skipped.
/// The batch's ignored files (`.env`), split into those still on disk and those removed.
fn ignored_changes(batch: &ide_vfs::FsBatch) -> (Vec<PathBuf>, Vec<PathBuf>) {
    batch.ignored.iter().cloned().partition(|p| p.symlink_metadata().is_ok())
}

fn record_external_changes(history: &LocalHistory, changed: &[PathBuf], removed: &[PathBuf]) {
    for path in changed.iter().take(HISTORY_BATCH_LIMIT) {
        if let Err(e) = history.record_file(path, "External change", false) {
            tracing::debug!("local history: {e}");
        }
    }
    if !removed.is_empty()
        && let Err(e) = history.note_removed(removed)
    {
        tracing::debug!("local history: {e}");
    }
}

/// Points `ide-vcs` at the git executable the user configured (empty = auto-detect).
pub fn apply_git_executable(settings: &ide_core::settings::Settings) {
    let exe = settings.vcs.git.executable.trim();
    ide_vcs::set_git_executable((!exe.is_empty()).then(|| Path::new(exe)));
}

pub fn history_retention(settings: &ide_core::settings::Settings) -> ide_workspace::Retention {
    ide_workspace::Retention {
        working_days: settings.files.local_history_days,
        max_bytes: u64::from(settings.files.local_history_max_mb.max(1)) * 1024 * 1024,
    }
}

/// Workspace members in manifest order, or extra Project Structure directories and the
/// Gradle project folders outside the project as of the last sync (`build_cache`).
fn content_roots(settings: &ide_core::settings::Settings, root: &Path) -> Vec<PathBuf> {
    if let Ok(Some(workspace)) = ide_workspace::workspace::Workspace::load(root) {
        return workspace.projects;
    }
    let mut out: Vec<PathBuf> = Vec::new();
    for p in settings.project_structure.content_roots.iter().map(|r| PathBuf::from(r.trim())) {
        if p.is_absolute() && p.is_dir() && !p.starts_with(root) && !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// The globs of files the project excludes (`files.exclude` plus linked roots).
fn exclude_globs(root: &Path, settings: &ide_core::settings::Settings, detected: &[SourceFolder]) -> Vec<String> {
    let _ = (root, detected);
    settings.file_excludes()
}

/// Stable FNV-1a directory name per project, for caches.
pub(crate) fn project_key(root: &Path) -> String {
    let s = root.to_string_lossy();
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// Records saved documents into Local History.
struct HistoryObserver(Arc<LocalHistory>);

impl DocumentObserver for HistoryObserver {
    fn saved(&self, _doc: ide_core::DocId, path: &Path, text: &str) {
        if let Err(e) = self.0.record(path, text, "Saved") {
            tracing::warn!("local history: {e}");
        }
    }
}

/// Watches the project folder: open documents follow disk changes and Local History
/// records external edits.
fn project_root_watcher(
    root: &Path,
    excludes: &[String],
    sink: &SharedSink,
    documents: &Arc<Documents>,
    history: &Arc<LocalHistory>,
    active: Arc<AtomicBool>,
    on_vcs_change: impl Fn() + Send + Sync + 'static,
) -> IdeResult<Watcher> {
    let (documents, history) = (documents.clone(), history.clone());
    Watcher::start(
        root,
        excludes.to_vec(),
        sink.clone(),
        Box::new(move |batch| {
            let changed: Vec<PathBuf> = batch.created.iter().chain(&batch.modified).cloned().collect();
            let (ignored, ignored_gone) = ignored_changes(batch);
            documents.on_disk_changed(&[&changed[..], &ignored].concat());
            record_external_changes(&history, &changed, &batch.removed);
            let _ = ignored_gone;
            // A repository appearing, a worktree added below the project or HEAD moving
            // changes what the Git views show, so rediscover them.
            if active.load(Ordering::Acquire) && changed.iter().chain(&batch.removed).any(|p| vcs_path_changed(p)) {
                on_vcs_change();
            }
        }),
    )
    .map_err(|e| IdeError::internal(format!("watcher {}: {e}", root.display())))
}

fn vcs_path_changed(path: &Path) -> bool {
    !path.ends_with(".git/index") || !ide_vcs::is_own_index_write(path)
}

#[cfg(test)]
mod index_event_tests {
    use super::vcs_path_changed;

    #[test]
    fn status_write_is_suppressed_but_external_index_and_refs_refresh() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git").current_dir(dir).args(args).output().unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        };
        git(&["init", "-q"]);
        let file = dir.join("file.txt");
        std::fs::write(&file, "original\n").unwrap();
        git(&["add", "file.txt"]);
        // Same contents, different stat data: status must refresh the index.
        std::fs::File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1))
            .unwrap();
        let index = dir.join(".git/index");
        let before = std::fs::read(&index).unwrap();
        let repo = ide_vcs::Repo::discover(dir).unwrap().unwrap();
        repo.status().unwrap();
        assert_ne!(before, std::fs::read(&index).unwrap(), "status must actually write the index");
        assert!(!vcs_path_changed(&index));
        assert!(!vcs_path_changed(&index), "duplicate notifications for the same identity");
        for path in [".git/HEAD", ".git/refs/heads/main", ".git/packed-refs"] {
            assert!(vcs_path_changed(&dir.join(path)));
        }
        std::fs::write(&file, "external staged change\n").unwrap();
        git(&["add", "file.txt"]);
        assert!(vcs_path_changed(&index));
        assert!(vcs_path_changed(&index));
        std::fs::remove_file(&index).unwrap();
        assert!(vcs_path_changed(&index));
    }
}

#[cfg(all(test, unix))]
mod workspace_edit_tests {
    use super::*;
    use ide_core::{FileEdit, LanguageId, Pos, Range, TextEdit, WorkspaceEdit};

    #[test]
    fn rename_via_symlink_updates_the_open_document_once() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("main.cpp");
        let alias = dir.path().join("alias.cpp");
        std::fs::write(&real, "add();\n").unwrap();
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        let documents = Documents::new(Arc::new(|_: IdeEvent| {}), Box::new(|_, _| LanguageId::new("cpp")));
        let doc = documents.open(&alias).unwrap();
        let changes = |path: PathBuf| FileEdit::Edit {
            path: path.into(),
            version: None,
            edits: vec![TextEdit { range: Range::new(Pos::new(0, 0), Pos::new(0, 3)), new_text: "sum_values".into() }],
        };
        documents
            .apply_workspace_edit(&WorkspaceEdit {
                files: vec![changes(real.clone()), changes(alias)],
                ..Default::default()
            })
            .unwrap();
        assert_eq!(std::fs::read_to_string(real).unwrap(), "sum_values();\n");
        assert_eq!(documents.snapshot(doc.id).unwrap().text, "sum_values();\n");
    }
}
