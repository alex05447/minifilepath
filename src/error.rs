use std::{
    error::Error,
    fmt::{Display, Formatter},
    path::PathBuf,
};

/// An error returned by methods which construct [`FilePath`](struct.FilePath.html)'s / [`FilePathBuf`](struct.FilePathBuf.html)'s.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FilePathError {
    /// Paths with prefixes are not allowed.
    PrefixedPath,
    /// Path contains an invalid component (root, current or parent directory).
    /// Contains the path to the invalid component.
    InvalidPathComponent(PathBuf),
    /// A path component is empty.
    /// Contains the path to the empty component.
    EmptyPathComponent(PathBuf),
    /// A path component contains invalid UTF-8.
    /// Contains the path to the invalid component.
    InvalidUTF8(PathBuf),
    /// Empty paths are not allowed.
    EmptyPath,
}

impl Error for FilePathError {}

impl Display for FilePathError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use FilePathError::*;

        match self {
            PrefixedPath => "paths with prefixes are not allowed".fmt(f),
            InvalidPathComponent(path) => write!(f, "path contains an invalid component at \"{:?}\" (root, current or parent directory)", path),
            EmptyPathComponent(path) => write!(f, "a path component at \"{:?}\" is empty", path),
            InvalidUTF8(path) => write!(f, "a path component at \"{:?}\" contains invalid UTF-8", path),
            EmptyPath => "empty paths are not allowed".fmt(f),
        }
    }
}
