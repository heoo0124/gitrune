//! Read-only document contents for a non-`file:` URI.
//!
//! A language server hands these out for library classes and decompiled code
//! (`jdt://contents/…`), and the Git history viewer hands them out for a file at a
//! revision. Both reach the editor the same way, so the type lives here rather than
//! in either producer.

use crate::ids::LanguageId;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Read-only contents a producer provides for a non-`file:` URI
/// (library class source, decompiled code, or a file at a Git revision).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VirtualDocument {
    pub uri: String,
    pub text: String,
    pub language_id: LanguageId,
    /// Tab title, e.g. `SpringApplication.class` (decompiled) or `SpringApplication.java`.
    pub title: String,
    /// Origin shown in the tooltip / banner, e.g. the jar path or `abc1234`.
    pub description: Option<String>,
    /// Generated Java from a decompiler, rather than real source or a disassembler stub.
    pub decompiled: bool,
}
