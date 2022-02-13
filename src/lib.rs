//! Some simple Rust wrapper types for non-empty, relative, case agnostic UTF-8 file system paths.

mod builder;
mod error;
mod iter;
mod path;
mod pathbuf;
mod util;

pub use {builder::*, error::*, path::*, pathbuf::*};
pub(crate) use {iter::*, util::*};

pub type FilePathComponent<'a> = &'a ministr::NonEmptyStr;

pub(crate) const SEPARATOR_CHAR: char = '/';

/// Maximum file path component length in bytes (in UTF-8 encoding).
pub const MAX_COMPONENT_LEN: usize = 255;

/// Maximum total file path length in bytes (in UTF-8 encoding), including the file path component separators.
pub const MAX_PATH_LEN: usize = 64 * 1024;

/// Attempts to validate the file path `component`.
///
/// Disallows
/// - current (`"."`) / parent (`".."`) directory components,
/// - components which end in a space (`' '`) or period (`'.'`),
/// - components which contain invalid characters (`'\'`, `'/'`, `':'`, `'*'`, `'?'`, `'"'`, `'<'`, `'>'`, `'|'`),
/// - components which are reserved file names (case-insensitive) or reserved file names with an extension
/// (`"AUX"`, `"COM?"`, `"CON"`, `"LPT?"`, `"NUL"`, `"PRN"`, where `?` is one of ASCII digits [`1` .. `9`]).
pub fn is_valid_path_component(component: FilePathComponent) -> bool {
    if component == "." {
        return false;
    } else if component == ".." {
        return false;
    } else {
        validate_normal_path_component(component, || std::path::PathBuf::new()).is_ok()
    }
}
