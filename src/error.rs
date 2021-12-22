use std::{
    error::Error,
    fmt::{Display, Formatter},
    path::PathBuf,
};

/// An error returned by methods which construct [`FilePath`](struct.FilePath.html)'s / [`FilePathBuf`](struct.FilePathBuf.html)'s.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FilePathError {
    /// Path contains a prefix.
    PrefixedPath,
    /// Path contains a root directory.
    RootDirectory,
    /// Path contains a current directory component.
    /// Contains the path to the invalid component.
    CurrentDirectory(PathBuf),
    /// Path contains a parent directory component.
    /// Contains the path to the invalid component.
    ParentDirectory(PathBuf),
    /// A path component is empty.
    /// Contains the path to the empty component.
    EmptyComponent(PathBuf),
    /// Path contains an invalid character.
    /// Contains the path to the invalid component and the invalid character.
    InvalidCharacter((PathBuf, char)),
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
            PrefixedPath => "paths contains a prefix".fmt(f),
            RootDirectory => "paths contains a root directory".fmt(f),
            CurrentDirectory(path) => write!(
                f,
                "path component at \"{:?}\" contains a current directory component",
                path
            ),
            ParentDirectory(path) => write!(
                f,
                "path component at \"{:?}\" contains a parent directory component",
                path
            ),
            EmptyComponent(path) => write!(f, "path component at \"{:?}\" is empty", path),
            InvalidCharacter((path, c)) => write!(
                f,
                "path component at \"{:?}\" contains an invalid character ('{}')",
                path, c
            ),
            InvalidUTF8(path) => {
                write!(f, "path component at \"{:?}\" contains invalid UTF-8", path)
            }
            EmptyPath => "empty paths are not allowed".fmt(f),
        }
    }
}
