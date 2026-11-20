//! The IDE backend without a UI toolkit: open projects per window, terminals,
//! processes, settings and the remote services. The desktop shell (`src-tauri`)
//! wraps one [`Host`] and adds windows and menus; `openidea-server`
//! runs one on a remote machine and serves it over JSON-RPC ([`rpc`]).

pub mod data_dirs;
pub mod environment;
pub mod ops;
mod project;
mod proxy;
mod requests;
pub mod shell_env;
pub mod trust;

pub use project::{Project, apply_git_executable, history_retention};

use ide_core::{CancellationToken, IdeError, IdeResult, RequestId, SharedSink};
use ide_workspace::SettingsStore;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Event sink for one window (by label).
pub type SinkFactory = Box<dyn Fn(&str) -> SharedSink + Send + Sync>;

pub struct HostConfig {
    pub config_dir: PathBuf,
    /// Persistent user state, including credentials and recovery history.
    pub data_dir: PathBuf,
    /// Machine-local tools and rebuildable caches.
    pub local_data_dir: PathBuf,
    pub sink_for: SinkFactory,
    /// The window that owns what `window` starts (e.g. a settings window's parent).
    pub owner: fn(&str) -> &str,
    pub runtime: tokio::runtime::Handle,
    /// Apply the open project's `devcontainer.json` (`remoteEnv`, `postStartCommand`) to
    /// this process's children: the remote server, which has one project.
    pub project_environment: bool,
}

pub struct Host {
    pub trust: trust::TrustStore,
    pub config_dir: PathBuf,
    /// Persistent user state, including credentials and recovery history.
    pub data_dir: PathBuf,
    /// Machine-local tools and rebuildable caches.
    pub local_data_dir: PathBuf,
    pub global_settings: SettingsStore,
    /// SSH connections of Settings › SSH Configurations.
    pub ssh: Arc<ide_ssh::SshManager>,
    /// What child processes get beyond this process's environment (Reload Environment, `remoteEnv`).
    pub environment: environment::Environment,
    /// The HTTP proxy's saved password (see `proxy.rs`).
    proxy_password: ide_ssh::secrets::SecretFile,
    /// The proxy password when it could not be saved (no credential store).
    proxy_session_password: Mutex<Option<String>>,
    pub(crate) sink_for: SinkFactory,
    owner: fn(&str) -> &str,
    pub runtime: tokio::runtime::Handle,
    /// One project per window, keyed by window label.
    projects: RwLock<HashMap<String, Arc<Project>>>,
    pub(crate) external_logs: RwLock<HashMap<String, Vec<Arc<ide_vcs::Repo>>>>,
    requests: requests::Requests,
    project_requests: RwLock<HashMap<String, Vec<RequestId>>>,
    /// Shutdowns of projects closed with [`Host::close_project_detached`].
    closing: parking_lot::Mutex<Vec<std::thread::JoinHandle<()>>>,
    /// One open at a time per window (see [`Host::opening`]).
    open_locks: parking_lot::Mutex<HashMap<String, Arc<parking_lot::Mutex<()>>>>,
}

/// How long exit waits for project shutdown before giving up.
const EXIT_SHUTDOWN_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

impl Host {
    pub fn new(config: HostConfig) -> Self {
        let HostConfig { config_dir, data_dir, local_data_dir, sink_for, owner, runtime, project_environment } = config;
        let global_settings = SettingsStore::load(&config_dir, None);
        let ssh = Arc::new(ide_ssh::SshManager::new(data_dir.join("ssh-passwords.json")));
        ssh.set_configs(global_settings.effective().ssh.configs);
        let host = Self {
            trust: trust::TrustStore::load(&config_dir, &data_dir),
            proxy_password: ide_ssh::secrets::SecretFile::new(data_dir.join("proxy-password.json")),
            proxy_session_password: Mutex::default(),
            ssh,
            environment: environment::Environment::new(project_environment, local_data_dir.clone()),
            global_settings,
            config_dir,
            data_dir,
            local_data_dir,
            sink_for,
            owner,
            runtime,
            projects: RwLock::default(),
            external_logs: RwLock::default(),
            requests: Default::default(),
            project_requests: RwLock::default(),
            closing: Default::default(),
            open_locks: Default::default(),
        };
        host.apply_proxy();
        host
    }

    /// Every open project (all windows).
    pub fn projects(&self) -> Vec<Arc<Project>> {
        self.projects.read().values().cloned().collect()
    }

    /// The project of `window` (or of the window owning it).
    pub fn project(&self, window: &str) -> IdeResult<Arc<Project>> {
        self.projects
            .read()
            .get((self.owner)(window))
            .cloned()
            .ok_or_else(|| IdeError::not_found("no project is open in this window"))
    }

    pub fn require_trust(&self, window: &str) -> IdeResult<()> {
        self.project(window)?.require_trust()
    }

    /// The window that owns what `window` starts.
    pub fn owner_window<'a>(&self, window: &'a str) -> &'a str {
        (self.owner)(window)
    }

    /// Event sink for things `window` starts (debug sessions, runs, terminals): its
    /// project's window-scoped sink, or a sink for that window when no project is open.
    pub fn window_sink(&self, window: &str) -> SharedSink {
        match self.project(window) {
            Ok(p) => p.sink.clone(),
            Err(_) => (self.sink_for)((self.owner)(window)),
        }
    }

    /// Closes `window`'s project and waits until its language servers, kernels and
    /// debug sessions have shut down. Never call this on the UI event-loop thread:
    /// the shutdown runs on the async runtime, whose workers may themselves be
    /// waiting for the event loop (IPC responses, window queries). Use
    /// [`Self::close_project_detached`] there.
    pub fn close_project(&self, window: &str) {
        if let Some(shutdown) = self.begin_close(window) {
            self.wait_blocking(shutdown);
        }
    }

    /// Like [`Self::close_project`], but returns once the project is detached from
    /// the window; its workers finish shutting down in the background, and
    /// [`Self::shutdown_notebooks_on_exit`] waits for them before the process exits.
    pub fn close_project_detached(&self, window: &str) {
        if let Some(shutdown) = self.begin_close(window) {
            let mut closing = self.closing.lock();
            closing.retain(|handle| !handle.is_finished());
            closing.push(shutdown);
        }
    }

    /// Detaches the project from `window` and stops what it started, synchronously,
    /// so the window can open another project at once. Returns the thread that
    /// finishes shutting down its workers.
    fn begin_close(&self, window: &str) -> Option<std::thread::JoinHandle<()>> {
        self.external_logs.write().remove(self.owner_window(window));
        for request in self.project_requests.write().remove(self.owner_window(window)).unwrap_or_default() {
            self.cancel_request(request);
        }
        let project = self.projects.write().remove(window)?;
        project.files_events_active.store(false, std::sync::atomic::Ordering::Release);
        if !project.light {
            self.environment.project_closed();
        }
        project.documents.clear_workspace_edit_history();
        Some(self.spawn_project_shutdown(Some(project)))
    }

    /// Project/window destruction must finish owned kernel cleanup, not enqueue
    /// work which can be lost as the desktop runtime exits: the returned thread is
    /// joined by the closer or, for a detached close, before exit.
    fn spawn_project_shutdown(&self, retired: Option<Arc<Project>>) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            // Dropping the project stops its file watcher and repository worker, which first
            // finish the batch they are applying, so it isn't done on the closing caller's
            // thread (the UI thread for a window close).
            drop(retired);
        })
    }

    /// Waits for detached closes, so a project opened next never races the
    /// language servers of one still shutting down (e.g. over a shared workspace).
    pub(crate) fn wait_detached_closes(&self) {
        let pending: Vec<_> = self.closing.lock().drain(..).collect();
        for shutdown in pending {
            self.wait_blocking(shutdown);
        }
    }

    fn wait_blocking(&self, shutdown: std::thread::JoinHandle<()>) {
        let wait = move || {
            if let Err(error) = shutdown.join() {
                tracing::error!("project shutdown thread failed: {error:?}");
            }
        };
        if tokio::runtime::Handle::try_current()
            .is_ok_and(|handle| handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread)
        {
            tokio::task::block_in_place(wait);
        } else {
            wait();
        }
    }

    /// Called before the desktop runtime exits, including update/restart exits, on
    /// the event-loop thread: waits for detached closes and the open projects'
    /// kernels, but no longer than [`EXIT_SHUTDOWN_WAIT`], so a runtime worker
    /// stuck on the event loop cannot keep the process from exiting.
    pub fn shutdown_notebooks_on_exit(&self) {
        let projects = self.projects();
        let mut pending: Vec<_> = self.closing.lock().drain(..).collect();
        pending.push(self.spawn_project_shutdown(projects.into_iter().next()));
        let deadline = std::time::Instant::now() + EXIT_SHUTDOWN_WAIT;
        while pending.iter().any(|handle| !handle.is_finished()) {
            if std::time::Instant::now() >= deadline {
                tracing::warn!("project shutdown did not finish within {EXIT_SHUTDOWN_WAIT:?}; exiting anyway");
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        for handle in pending {
            if let Err(error) = handle.join() {
                tracing::error!("project shutdown thread failed: {error:?}");
            }
        }
    }

    /// Called on desktop exit. GitRune starts no long-lived child processes outside a
    /// project's environment, which `shutdown` already stops.
    pub fn stop_processes_on_exit(&self) {}

    /// Called before desktop exit, which may bypass document destructors.
    pub fn clear_workspace_edit_history_on_exit(&self) {
        for project in self.projects() {
            project.documents.clear_workspace_edit_history();
        }
    }

    /// Stops what this host started: child processes of the project environment.
    pub async fn shutdown(&self) {
        self.environment.shutdown();
        let projects: Vec<_> = self.projects.write().drain().map(|(_, p)| p).collect();
        for project in &projects {
            project.files_events_active.store(false, std::sync::atomic::Ordering::Release);
        }
        for project in &projects {
            project.documents.clear_workspace_edit_history();
        }
    }

    // ---- cancellable requests: the UI allocates ids, `cancel_request` cancels ----

    pub fn begin_request(&self, id: RequestId) -> CancellationToken {
        self.requests.begin(id)
    }

    pub fn begin_project_request(&self, window: &str, id: RequestId) -> CancellationToken {
        let token = self.begin_project_read_request(window, id);
        if self.project(window).is_ok_and(|p| !p.is_trusted()) {
            token.cancel();
        }
        token
    }

    /// Read-only work remains cancellable/disposed with its window in restricted projects.
    pub fn begin_project_read_request(&self, window: &str, id: RequestId) -> CancellationToken {
        let token = self.begin_request(id);
        self.project_requests.write().entry(self.owner_window(window).to_owned()).or_default().push(id);
        token
    }

    pub fn end_request(&self, id: RequestId) {
        for ids in self.project_requests.write().values_mut() {
            ids.retain(|entry| *entry != id);
        }
        self.requests.end(id);
    }

    /// Cancels request `id`, also when it has not begun yet (it then starts out cancelled).
    pub fn cancel_request(&self, id: RequestId) {
        self.requests.cancel(id);
    }

    /// The lock an open of `window` holds from its check for the current project to its
    /// insert: two opens racing (a double click, a launch path and a restore) would each start
    /// an index, language servers and watchers, and the second would replace the first unclosed.
    pub(crate) fn opening(&self, window: &str) -> Arc<parking_lot::Mutex<()>> {
        self.open_locks.lock().entry(window.to_owned()).or_default().clone()
    }

    pub(crate) fn insert_project(&self, window: &str, project: Arc<Project>) {
        self.projects.write().insert(window.to_owned(), project);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn detached_close_releases_the_project_before_a_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let base = dunce::canonicalize(temp.path()).unwrap();
        let root = base.join("project");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();
        let host = Host::new(HostConfig {
            config_dir: base.join("config"),
            data_dir: base.join("data"),
            local_data_dir: base.join("local"),
            sink_for: Box::new(|_| Arc::new(|_| {})),
            owner: |w| w,
            runtime: tokio::runtime::Handle::current(),
            project_environment: false,
        });
        let project = Arc::downgrade(&host.open_project("main", &root).unwrap());
        host.close_project_detached("main");
        assert!(host.project("main").is_err());
        // The project (its file watcher and index database) is dropped off the closing thread, but
        // before opening a project waits out the close.
        host.wait_detached_closes();
        assert!(project.upgrade().is_none(), "the closed project is dropped");
        host.open_project("main", &root).unwrap();
        host.close_project("main");
    }
}
