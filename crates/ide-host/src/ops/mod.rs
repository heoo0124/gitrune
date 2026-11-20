//! Command implementations shared by the desktop shell's IPC commands and the
//! typed IPC commands: each takes the [`Host`], the calling window and the
//! command's arguments, and streams through [`Emit`] callbacks.

pub mod bytes;
pub mod doc;
pub mod env;
pub mod fs;
pub mod history;
pub mod project;
pub mod vcs;

use crate::Host;
use ide_core::{IdeError, IdeResult};
use std::sync::Arc;

/// A stream of values to the caller (a Tauri channel, or channel notifications).
pub type Emit<T> = Arc<dyn Fn(T) + Send + Sync>;

/// A streaming argument: what the desktop shell passes as a Tauri channel and
/// the server as `channel` notifications.
pub struct Channel<T>(Emit<T>);

impl<T> Channel<T> {
    pub fn new(send: impl Fn(T) + Send + Sync + 'static) -> Self {
        Self(Arc::new(send))
    }

    /// Sends one value; never fails (a closed receiver drops it).
    pub fn send(&self, value: T) -> IdeResult<()> {
        (self.0)(value);
        Ok(())
    }
}

impl<T> Clone for Channel<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Host {
    /// Runs blocking work (disk I/O, parsing, large copies) on the blocking pool.
    pub async fn blocking<T: Send + 'static>(&self, f: impl FnOnce() -> IdeResult<T> + Send + 'static) -> IdeResult<T> {
        self.runtime.spawn_blocking(f).await.map_err(|e| IdeError::internal(e.to_string()))?
    }
}
