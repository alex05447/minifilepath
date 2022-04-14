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
    /// A path component length in bytes is longer than `MAX_COMPONENT_LEN`.
    /// Contains the path to the component and its length in bytes.
    ComponentTooLong((PathBuf, usize)),
    /// Path component contains an invalid character.
    /// Contains the path to the invalid component and the invalid character.
    InvalidCharacter((PathBuf, char)),
    /// Path component ends with a period.
    /// Contains the path to the invalid component.
    ComponentEndsWithAPeriod(PathBuf),
    /// Path component ends with a space.
    /// Contains the path to the invalid component.
    ComponentEndsWithASpace(PathBuf),
    /// Path component contains a reserved file name.
    /// Contains the path to the invalid component.
    ReservedName(PathBuf),
    /// A path component contains invalid UTF-8.
    /// Contains the path to the invalid component.
    InvalidUTF8(PathBuf),
    /// Empty paths are not allowed.
    EmptyPath,
    /// Path length in bytes is longer than `MAX_PATH_LEN`.
    /// Contains the length of the path in bytes.
    PathTooLong(usize),
}

impl Error for FilePathError {}

impl Display for FilePathError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use FilePathError::*;

        match self {
            PrefixedPath => "path contains a prefix".fmt(f),
            RootDirectory => "path contains a root directory".fmt(f),
            CurrentDirectory(path) => write!(
                f,
                "path component at {:?} contains a current directory component",
                path
            ),
            ParentDirectory(path) => write!(
                f,
                "path component at {:?} contains a parent directory component",
                path
            ),
            EmptyComponent(path) => write!(f, "path component at {:?} is empty", path),
            ComponentTooLong((path, len)) => write!(
                f,
                "path component at {:?} is too long ({} bytes)",
                path, len
            ),
            InvalidCharacter((path, c)) => write!(
                f,
                "path component at {:?} contains an invalid character ('{}')",
                path, c
            ),
            ComponentEndsWithAPeriod(path) => {
                write!(f, "path component at {:?} ends with a period", path)
            }
            ComponentEndsWithASpace(path) => {
                write!(f, "path component at {:?} ends with a space", path)
            }
            ReservedName(path) => write!(
                f,
                "path component at {:?} contains a reserved name",
                path
            ),
            InvalidUTF8(path) => {
                write!(f, "path component at {:?} contains invalid UTF-8", path)
            }
            EmptyPath => "empty paths are not allowed".fmt(f),
            PathTooLong(len) => write!(f, "path is too long ({} bytes)", len),
        }
    }
}
