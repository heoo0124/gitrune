#[allow(unused_imports)]
use super::*;
use crate::Host;
use ide_core::{DocId, IdeError, IdeResult};
use ide_text::LineEnding;
use ide_workspace::HistoryEntry;
use std::path::PathBuf;

/// Runs blocking work (disk I/O, parsing, large copies) on the blocking pool,
/// keeping it off the main thread and the async runtime's workers.
pub async fn blocking<T: Send + 'static>(f: impl FnOnce() -> IdeResult<T> + Send + 'static) -> IdeResult<T> {
    tokio::task::spawn_blocking(f).await.map_err(|e| IdeError::internal(e.to_string()))?
}

/// The CodeMirror mode `path` gets when opened (`DocumentInfo::ui_mode`),
/// without opening it: previews of closed files.
pub async fn doc_ui_mode(host: &Host, window: &str, path: PathBuf) -> IdeResult<Option<String>> {
    let project = host.project(window)?;
    blocking(move || Ok(project.documents.ui_mode_of(&path))).await
}

/// "Reload with Encoding" (`reload`) or "Convert to Encoding"; returns the new encoding label.
pub async fn doc_set_encoding(
    host: &Host,
    window: &str,
    doc: DocId,
    encoding: String,
    reload: bool,
) -> IdeResult<String> {
    let project = host.project(window)?;
    host.blocking(move || project.documents.set_encoding(doc, &encoding, reload)).await
}

pub fn doc_set_line_ending(host: &Host, window: &str, doc: DocId, line_ending: LineEnding) -> IdeResult<()> {
    host.project(window)?.documents.set_line_ending(doc, line_ending)
}

pub fn history_list(host: &Host, window: &str, path: PathBuf) -> IdeResult<Vec<HistoryEntry>> {
    host.project(window)?.history.list(&path)
}

pub fn history_content(host: &Host, window: &str, id: String) -> IdeResult<String> {
    host.project(window)?.history.content(&id)
}
