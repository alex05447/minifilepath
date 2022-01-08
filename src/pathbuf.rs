use {
    crate::*,
    ministr::{NonEmptyStr, NonEmptyString},
    std::{
        borrow::Borrow,
        fmt::{Display, Formatter},
        hash::{Hash, Hasher},
        iter::DoubleEndedIterator,
        ops::Deref,
        path::{Path, PathBuf},
    },
};

/// Non-empty, relative, case agnostic UTF-8 file system path.
/// Every [`FilePathBuf`] is a valid [`PathBuf`], but not vice-versa.
///
/// The string representation contains nothing but normal path components.
/// Always uses forward slashes as path component separators.
///
/// NOTE: [`FilePath`]'s are considered equal if they produce the same [`components`](#method.components),
/// or, equivalently, if the underlying strings are equal.
///
/// Hashed componentwise, not as the string representation.
///
/// E.g.: "foo/βαρ/Baz BoB.txt", "textures/props/barrels/red_barrel.png".
/// But not "/foo/bar/", or "C:\Bill\Amy.cfg", or "../meshes/props/barrels/red_barrel.fbx".
///
/// This is the owned version, [`FilePath`] is the borrowed version.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FilePathBuf(pub(crate) NonEmptyString);

impl FilePathBuf {
    /// Tries to create a [`FilePathBuf`] directly from a [`path`](Path).
    ///
    /// Returns an [`error`](FilePathError) if the [`path`](Path) is not a valid [`FilePathBuf`].
    ///
    /// You can also build a [`FilePathBuf`] using a [`FilePathBuilder`].
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, FilePathError> {
        let path = path.as_ref();
        let mut builder = FilePathBuilder::with_capacity(path.as_os_str().len());
        builder.push(path)?;
        builder.build().ok_or(FilePathError::EmptyPath)
    }

    /// Creates a [`FilePathBuf`] directly from a `path` string.
    ///
    /// # Safety
    ///
    /// The caller guarantees the `path` is a valid [`FilePathBuf`].
    ///
    /// # Panics
    ///
    /// In debug configuration only, panics if `path` is not a valid [`FilePathBuf`].
    pub unsafe fn new_unchecked(path: String) -> Self {
        debug_assert!(
            Self::is_valid_filepath(&path),
            "tried to create a `FilePathBuf` from an invalid path"
        );
        Self(NonEmptyString::new_unchecked(path))
    }

    /// Converts the [`FilePathBuf`] back to a [`FilePathBuilder`], also clearing it,
    /// allowing the buffer to be reused.
    pub fn into_builder(self) -> FilePathBuilder {
        FilePathBuilder::from(self.0.into_inner())
    }

    pub fn into_path(self) -> PathBuf {
        PathBuf::from(self.0.into_inner())
    }

    pub fn into_ne_string(self) -> NonEmptyString {
        self.0
    }

    pub fn into_string(self) -> String {
        self.0.into_inner()
    }

    pub fn as_file_path(&self) -> &FilePath {
        // It is safe to directly convert a `NonEmptyStr` with a valid path to a `FilePath`.
        unsafe { FilePath::from_str(self.0.as_ne_str()) }
    }

    pub fn as_path(&self) -> &Path {
        Path::new(self.0.as_str())
    }

    pub fn as_ne_str(&self) -> &NonEmptyStr {
        self.0.as_ne_str()
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Returns an iterator over the (non-empty, UTF-8 string) components of the [`FilePathBuf`], root to leaf.
    ///
    /// NOTE: can be reversed via `rev()` to iterate leaf to root.
    pub fn components(&self) -> impl DoubleEndedIterator<Item = FilePathComponent> {
        // Unlike `FilePath`, we may use the simpler iterator because of the `FilePathBuf`'s canonical string representation.
        FilePathIter::new(self.as_file_path())
    }

    #[cfg(debug_assertions)]
    fn is_valid_filepath(path: &str) -> bool {
        if let Some(path_) = Self::new(&path).ok() {
            path_.as_str() == path
        } else {
            false
        }
    }
}

impl AsRef<FilePath> for FilePathBuf {
    fn as_ref(&self) -> &FilePath {
        self.as_file_path()
    }
}

impl Deref for FilePathBuf {
    type Target = FilePath;

    fn deref(&self) -> &Self::Target {
        self.as_file_path()
    }
}

impl Borrow<FilePath> for FilePathBuf {
    fn borrow(&self) -> &FilePath {
        self.as_file_path()
    }
}

impl Hash for FilePathBuf {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_file_path().hash(state)
    }
}

impl Display for FilePath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(non_snake_case)]
    fn EmptyPath() {
        assert_eq!(
            FilePathBuf::new("").err().unwrap(),
            FilePathError::EmptyPath
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn PrefixedPath() {
        assert_eq!(
            FilePathBuf::new("C:/foo").err().unwrap(),
            FilePathError::PrefixedPath
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn RootDirectory() {
        assert_eq!(
            FilePathBuf::new("/foo").err().unwrap(),
            FilePathError::RootDirectory
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn CurrentDirectory() {
        assert_eq!(
            FilePathBuf::new("./foo\\baz").err().unwrap(),
            FilePathError::CurrentDirectory(PathBuf::new())
        );
        // But this works:
        let foobaz = FilePathBuf::new("foo\\.\\baz").unwrap();
        assert_eq!(foobaz.to_owned(), FilePathBuf::new("foo/baz").unwrap());
    }

    #[test]
    #[allow(non_snake_case)]
    fn ParentDirectory() {
        assert_eq!(
            FilePathBuf::new("..\\foo").err().unwrap(),
            FilePathError::ParentDirectory(PathBuf::new())
        );
        assert_eq!(
            FilePathBuf::new("foo/..").err().unwrap(),
            FilePathError::ParentDirectory(PathBuf::from("foo"))
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn EmptyPathComponent() {
        // Repeated path separators are ignored and thus do not generate an empty path component.
        let foobaz = FilePathBuf::new("foo\\\\baz").unwrap();
        assert_eq!(foobaz.to_owned(), FilePathBuf::new("foo/baz").unwrap());

        let foobaz = FilePathBuf::new("foo//baz").unwrap();
        assert_eq!(foobaz.to_owned(), FilePathBuf::new("foo/baz").unwrap());
    }

    #[test]
    #[allow(non_snake_case)]
    fn InvalidCharacter() {
        assert_eq!(
            FilePathBuf::new("foo\\a?").err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::from("foo\\a?"), '?'))
        );
        assert_eq!(
            FilePathBuf::new("foo/BAR/*").err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::from("foo/BAR/*"), '*'))
        );
        assert_eq!(
            FilePathBuf::new("foo/bar<1>").err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::from("foo/bar<1>"), '<'))
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn ComponentEndsWithAPeriod() {
        assert_eq!(
            FilePathBuf::new("foo\\...").err().unwrap(),
            FilePathError::ComponentEndsWithAPeriod(PathBuf::from("foo\\..."))
        );
        // But this is a parent directory.
        assert_eq!(
            FilePathBuf::new("foo\\..").err().unwrap(),
            FilePathError::ParentDirectory(PathBuf::from("foo"))
        );
        // And this is a current directory.
        assert_eq!(
            FilePathBuf::new("./foo").err().unwrap(),
            FilePathError::CurrentDirectory(PathBuf::new())
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn ComponentEndsWithASpace() {
        assert_eq!(
            FilePathBuf::new("foo\\bar.txt ").err().unwrap(),
            FilePathError::ComponentEndsWithASpace(PathBuf::from("foo\\bar.txt "))
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn ReservedName() {
        assert_eq!(
            FilePathBuf::new("foo\\NUL").err().unwrap(),
            FilePathError::ReservedName(PathBuf::from("foo\\NUL"))
        );
        assert_eq!(
            FilePathBuf::new("BAR/com7").err().unwrap(),
            FilePathError::ReservedName(PathBuf::from("BAR/com7"))
        );
    }

    #[test]
    fn components() {
        // `.` in the middle is ignored.
        // Repeated path separators are ignored.
        // Trailing path separators are ignored.
        let path = FilePathBuf::new("foo/./bar//Baz\\\\BILL\\").unwrap();
        assert_eq!(path, FilePathBuf::new("foo/bar/Baz/BILL").unwrap());
        for (idx, component) in path.components().enumerate() {
            match idx {
                0 => assert_eq!(component.as_str(), "foo"),
                1 => assert_eq!(component.as_str(), "bar"),
                2 => assert_eq!(component.as_str(), "Baz"),
                3 => assert_eq!(component.as_str(), "BILL"),
                _ => panic!(),
            }
        }
        for (idx, component) in path.components().rev().enumerate() {
            match idx {
                0 => assert_eq!(component.as_str(), "BILL"),
                1 => assert_eq!(component.as_str(), "Baz"),
                2 => assert_eq!(component.as_str(), "bar"),
                3 => assert_eq!(component.as_str(), "foo"),
                _ => panic!(),
            }
        }
    }

    #[test]
    fn equality() {
        let l = FilePathBuf::new("foo/./bar//Baz\\\\BILL\\").unwrap();
        let r = FilePathBuf::new("foo/bar/Baz/BILL").unwrap();
        assert_eq!(l, r);
        assert_eq!(l.as_path(), r.as_path());
        // Strings and hashes are equal.
        assert_eq!(l.as_str(), r.as_str());
        let mut hl = std::collections::hash_map::DefaultHasher::new();
        let mut hr = hl.clone();
        l.hash(&mut hl);
        r.hash(&mut hr);
        assert_eq!(hl.finish(), hr.finish());
    }
}
