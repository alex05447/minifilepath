//! # minifilepath
//!
//! Some simple Rust wrapper types for non-empty, relative, case-sensitive, UTF-8 file paths.

mod builder;
mod error;
mod iter;
mod path;
mod pathbuf;
mod util;

pub(crate) use util::*;
pub use {
    builder::*,
    error::*,
    iter::{FilePathBufIter, FilePathIter},
    path::*,
    pathbuf::*,
};

pub type FilePathComponent<'a> = &'a ministr::NonEmptyStr;

pub const SEPARATOR_CHAR: char = '/';
pub const SEPARATOR_BYTE: u8 = b'/';

/// Maximum file path component length in bytes (in UTF-8 encoding).
pub const MAX_COMPONENT_LEN: usize = u8::MAX as usize;

/// Maximum total file path length in bytes (in UTF-8 encoding), including the file path component separators.
pub const MAX_PATH_LEN: usize = u16::MAX as usize;

/// Maximum number of components a file path may have.
pub const MAX_NUM_COMPONENTS: usize = MAX_PATH_LEN / 2; // `MAX_PATH_LEN == 8` -> "a/a/a/ab", `MAX_NUM_COMPONENTS == 4 == MAX_PATH_LEN / 2`

use {
    ministr::NonEmptyStr,
    miniunchecked::*,
    std::{path::PathBuf, str},
};

/// Attempts to validate the file path `component`.
///
/// Disallows
/// - current (`"."`) / parent (`".."`) directory components,
/// - components which end in a space (`' '`) or period (`'.'`),
/// - components which contain invalid characters (`'\'`, `'/'`, `':'`, `'*'`, `'?'`, `'"'`, `'<'`, `'>'`, `'|'`) or ASCII control characters,
/// - components which are reserved file names (case-insensitive) or reserved file names with an extension
/// (`"AUX"`, `"COM?"`, `"CON"`, `"LPT?"`, `"NUL"`, `"PRN"`, where `?` is one of ASCII digits [`1` .. `9`]).
pub fn is_valid_path_component(component: FilePathComponent<'_>) -> bool {
    if component == "." {
        return false;
    } else if component == ".." {
        return false;
    } else {
        validate_path_component(component, || PathBuf::new()).is_ok()
    }
}

/// Result of splitting a file path component into the file stem and extension parts.
///
/// Returned when the path component has an extension and a (maybe empty) file stem.
/// E.g.:
/// - "foo.txt" -> { file_stem: Some("foo"), extension: "txt" }
/// - ".gitignore" -> { file_stem: None, extension: "gitgnore" } (NOTE: this is different from standard library behaviour)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FileStemAndExtension<'a> {
    pub file_stem: Option<FilePathComponent<'a>>,
    pub extension: FilePathComponent<'a>,
}

/// Splits the `file_name` into the file stem and extension parts.
///
/// Returns `None` if the `file_name` does not have an extension (or if it somehow ends with a period).
/// Returns `Some` if the `file_name` has an extension and a (maybe empty) file stem.
///
/// NOTE: this differs from the standard library w.r.t. path components which start with a period.
/// Standard library considers a file_name like `".gitignore"` to have a file stem part `".gitignore"` and no extension.
/// This function simply treats anything past the last period as an extension, always.
///
/// E.g.:
/// - `".txt"` -> `Some((None, "txt"))` (NOTE: not `None`)
/// - `"foo.txt"` -> `Some((Some("foo"), "txt"))`
/// - `"foo.bar.txt"` -> `Some((Some("foo.bar"), "txt"))`
/// - `"foo."` -> invalid path (cannot end with a period), but this returns `None`
/// - `"foo"` -> `None`
pub fn file_stem_and_extension(
    file_name: FilePathComponent<'_>,
) -> Option<FileStemAndExtension<'_>> {
    let mut iter = file_name.as_str().as_bytes().rsplitn(2, |b| *b == b'.');
    let extension = unsafe {
        iter.next()
            .unwrap_unchecked_dbg_msg("`FilePathComponent`'s must be non-empty")
    };

    if let Some(file_name) = iter.next() {
        // ".txt" -> ("", "txt") -> `Some((None, "txt"))`
        // "foo.txt" -> ("foo", "txt") -> `Some((Some("foo"), "txt"))`
        // "foo.bar.txt" -> ("foo.bar", "txt") -> `Some((Some("foo.bar"), "txt"))`
        if let Some(extension) = NonEmptyStr::new(unsafe { str::from_utf8_unchecked(extension) }) {
            Some(FileStemAndExtension {
                file_stem: NonEmptyStr::new(unsafe { str::from_utf8_unchecked(file_name) }),
                extension,
            })

        // "foo." -> invalid path (cannot end with a period), but this returns `None`
        } else {
            None
        }

    // "foo" -> ("foo", None) -> `None`
    } else {
        None
    }
}

/// Contains the [`FilePath`]'s file path and file name parts, for [`FilePath`]s which have them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FilePathAndName<'a> {
    /// The file path part of the [`FilePath`].
    /// E.g. `"foo/bar"` -> `"foo"`, `"foo/bar/baz.txt"` -> `"foo/bar"`.
    pub file_path: &'a FilePath,
    /// The file name part of the [`FilePath`].
    /// E.g. `"foo/bar"` -> `"bar"`, `"foo/bar/baz.txt"` -> `"baz.txt"`.
    pub file_name: FilePathComponent<'a>,
}

/// Splits the `file_path` into the file path and file name parts.
///
/// E.g. `"foo/bar"` -> `Some{ "foo", "bar" }`, `"foo/bar/baz.txt"` -> `Some{ "foo/bar", "baz.txt" }`, `"bill.cfg"` -> `None`.
///
/// Returns `None` if the `file_path` does not have the file path part (i.e. it's just a file name).
pub fn file_path_and_name(file_path: &FilePathBuf) -> Option<FilePathAndName<'_>> {
    let mut iter = file_path
        .as_str()
        .as_bytes()
        .rsplitn(2, |b| *b == SEPARATOR_BYTE);

    let file_name = unsafe {
        iter.next()
            .unwrap_unchecked_dbg_msg("`FilePathBuf`'s must be non-empty")
    };

    iter.next().map(|file_path| FilePathAndName {
        file_path: unsafe { FilePath::new_unchecked(str::from_utf8_unchecked(file_path)) },
        file_name: unsafe { NonEmptyStr::new_unchecked(str::from_utf8_unchecked(file_name)) },
    })
}

#[cfg(test)]
mod tests {
    use {super::*, ministr_macro::nestr};

    #[test]
    fn file_name_and_extension_test() {
        assert_eq!(
            file_stem_and_extension(nestr!(".txt")),
            Some(FileStemAndExtension {
                file_stem: None,
                extension: nestr!("txt")
            })
        );
        assert_eq!(
            file_stem_and_extension(nestr!("foo.txt")),
            Some(FileStemAndExtension {
                file_stem: Some(nestr!("foo")),
                extension: nestr!("txt")
            })
        );
        assert_eq!(
            file_stem_and_extension(nestr!("foo.bar.txt")),
            Some(FileStemAndExtension {
                file_stem: Some(nestr!("foo.bar")),
                extension: nestr!("txt")
            })
        );
        assert_eq!(file_stem_and_extension(nestr!("foo")), None,);
        assert_eq!(file_stem_and_extension(nestr!("foo.")), None,);
    }

    #[test]
    fn file_path_and_name_test() {
        assert_eq!(file_path_and_name(&FilePathBuf::new(".txt").unwrap()), None);
        assert_eq!(
            file_path_and_name(&FilePathBuf::new("foo.txt").unwrap()),
            None
        );
        assert_eq!(
            file_path_and_name(&FilePathBuf::new("foo.bar.txt").unwrap()),
            None
        );
        assert_eq!(file_path_and_name(&FilePathBuf::new("foo").unwrap()), None);
        //assert_eq!(file_path_and_name(&FilePathBuf::new("foo.").unwrap()), None); // ComponentEndsWithAPeriod

        assert_eq!(file_path_and_name(&FilePathBuf::new("foo/").unwrap()), None);
        //assert_eq!(file_path_and_name(&FilePathBuf::new("/foo").unwrap()), None); // RootDirectory

        assert_eq!(
            file_path_and_name(&FilePathBuf::new("foo/bar").unwrap()),
            Some(FilePathAndName {
                file_path: FilePath::new("foo").unwrap(),
                file_name: nestr!("bar")
            })
        );
        assert_eq!(
            file_path_and_name(&FilePathBuf::new("foo bar/baz bob").unwrap()),
            Some(FilePathAndName {
                file_path: FilePath::new("foo bar").unwrap(),
                file_name: nestr!("baz bob")
            })
        );
        assert_eq!(
            file_path_and_name(&FilePathBuf::new("foo/bar/baz bob").unwrap()),
            Some(FilePathAndName {
                file_path: FilePath::new("foo/bar").unwrap(),
                file_name: nestr!("baz bob")
            })
        );
        assert_eq!(
            file_path_and_name(&FilePathBuf::new("foo/bar.txt").unwrap()),
            Some(FilePathAndName {
                file_path: FilePath::new("foo").unwrap(),
                file_name: nestr!("bar.txt")
            })
        );
        assert_eq!(
            file_path_and_name(&FilePathBuf::new("foo/.txt").unwrap()),
            Some(FilePathAndName {
                file_path: FilePath::new("foo").unwrap(),
                file_name: nestr!(".txt")
            })
        );
    }
}
