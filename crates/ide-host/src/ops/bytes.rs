//! Raw file bytes for the image viewer and hex view (see `ide_vfs::bytes`).
use crate::Host;

use crate::ops::doc::blocking;
use ide_core::IdeResult;
use ide_vfs::{ByteRange, FileProbe};
use std::path::PathBuf;

/// Size and text/binary classification, from the file's head only.
pub async fn fs_probe(_host: &Host, _window: &str, path: PathBuf) -> IdeResult<FileProbe> {
    blocking(move || ide_vfs::probe(&path)).await
}

/// Up to `len` bytes from `offset` (capped per call), base64-encoded.
pub async fn fs_read_range(_host: &Host, _window: &str, path: PathBuf, offset: u64, len: u32) -> IdeResult<ByteRange> {
    blocking(move || ide_vfs::read_range(&path, offset, len)).await
}

/// An image file as a `data:` URL.
pub async fn fs_read_image(_host: &Host, _window: &str, path: PathBuf) -> IdeResult<String> {
    blocking(move || ide_vfs::read_data_url(&path)).await
}
