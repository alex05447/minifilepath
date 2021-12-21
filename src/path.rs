use {
    crate::*,
    ministr::{NonEmptyStr, NonEmptyString},
    std::{
        borrow::ToOwned,
        cmp::PartialEq,
        ffi::OsStr,
        fmt::{Display, Formatter},
        hash::{Hash, Hasher},
        iter::{DoubleEndedIterator, Iterator},
        path::Path,
    },
};

/// Non-empty, relative, case agnostic UTF-8 file system path.
/// Every [`FilePath`] is a valid [`Path`], but not vice-versa.
///
/// If [`created`](#method.try_from) directly from a [`Path`]:
/// 1) may use a platform-specific path separator (backslash or a forward slash);
/// 2) may contain repeated path separators;
/// 3) may contain mid-path "current directory" components (`.`).
///
/// NOTE: [`FilePath`]'s are considered equal if they produce the same [`components`](#method.components),
/// even if the underlying strings are not equal (i.e. similar to [`std::path::Path`]).
///
/// Hashed componentwise, not as the string representation.
///
/// E.g.: "foo//βαρ/../Baz BoB.txt", "textures\.\props\barrels\red_barrel.png".
/// But not "/foo/bar", or "C:\Bill\Amy.cfg", or "../meshes/props/barrels/red_barrel.fbx".
///
/// This is the borrowed version, [`FilePathBuf`] is the owned version.
#[derive(Debug)]
pub struct FilePath(pub(crate) NonEmptyStr);

impl FilePath {
    /// Tries to create a [`FilePath`] directly from a [`path`](Path).
    ///
    /// Returns an [`error`](FilePathError) if the [`path`](Path) is not a valid [`FilePath`].
    pub fn new<P: AsRef<Path> + ?Sized>(path: &P) -> Result<&Self, FilePathError> {
        if Self::is_valid_filepath(path.as_ref()) {
            // We validated it, so it's safe to convert the path directly to a (non-empty) UTF-8 string slice.
            Ok(unsafe { Self::from_path(path.as_ref()) })
        } else {
            Err(FilePathError::EmptyPath)
        }
    }

    /// Creates a [`FilePath`] directly from a [`path`](Path).
    ///
    /// # Safety
    ///
    /// The caller guarantees the `path` is a valid [`FilePath`].
    ///
    /// # Panics
    ///
    /// In debug configuration only, panics if `path` is not a valid [`FilePath`].
    pub unsafe fn new_unchecked<P: AsRef<Path> + ?Sized>(path: &P) -> &Self {
        debug_assert!(
            Self::is_valid_filepath(path.as_ref()),
            "tried to create a `FilePath` from an invalid path"
        );
        Self::from_path(path.as_ref())
    }

    pub fn as_path(&self) -> &Path {
        Path::new(self.0.as_str())
    }

    pub fn as_ne_str(&self) -> &NonEmptyStr {
        &self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Returns an iterator over the (non-empty, UTF-8 string) components of the [`FilePath`], root to leaf.
    ///
    /// NOTE: can be reversed via `rev()` to iterate leaf to root.
    pub fn components(&self) -> impl DoubleEndedIterator<Item = &NonEmptyStr> {
        // Need to use `PathIter` instead of `FilePathIter` because of `std::path::Path` quirks, see the comments for `FilePath`.
        PathIter::new(self)
    }

    /// The caller guarantees `path` is a valid file path.
    /// In this case it is safe to directly convert a `NonEmptyStr` to a `FilePath`.
    pub(crate) unsafe fn from_str(path: &NonEmptyStr) -> &Self {
        &*(path.as_str() as *const str as *const FilePath)
    }

    /// The caller guarantees `path` is a valid non-empty UTF-8 string slice.
    /// In this case it is safe to directly convert a non-empty UTF-8 `OsStr` to a `FilePath`.
    unsafe fn from_path(path: &Path) -> &Self {
        debug_assert!(!path.as_os_str().is_empty());
        &*(path.as_os_str() as *const OsStr as *const str as *const FilePath)
    }

    fn is_valid_filepath(path: &Path) -> bool {
        match iterate_path(path, |_| {}) {
            Ok(true) => true,
            _ => false,
        }
    }
}

impl AsRef<FilePath> for FilePath {
    fn as_ref(&self) -> &FilePath {
        &self
    }
}

impl ToOwned for FilePath {
    type Owned = FilePathBuf;

    fn to_owned(&self) -> Self::Owned {
        let mut string = String::with_capacity(self.0.len());
        append_file_path_to_string(self, &mut string);
        FilePathBuf(unsafe { NonEmptyString::new_unchecked(string) })
    }
}

impl Hash for FilePath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for component in self.components() {
            state.write(component.as_bytes());
        }
    }
}

impl PartialEq<Self> for FilePath {
    fn eq(&self, other: &Self) -> bool {
        // Similar to `std::path::Path`, comparing leaf-to-root.
        Iterator::eq(self.components().rev(), other.components().rev())
    }
}

impl Eq for FilePath {}

impl Display for FilePathBuf {
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
        assert_eq!(FilePath::new("").err().unwrap(), FilePathError::EmptyPath);
    }

    #[test]
    #[allow(non_snake_case)]
    fn PrefixedPath() {
        assert_eq!(
            FilePath::new("C:/foo").err().unwrap(),
            FilePathError::PrefixedPath
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn InvalidPathComponent() {
        // `RootDir`
        assert_eq!(
            FilePath::new("/foo").err().unwrap(),
            FilePathError::InvalidPathComponent(PathBuf::new())
        );

        // `ParentDir`
        assert_eq!(
            FilePath::new("..\\foo").err().unwrap(),
            FilePathError::InvalidPathComponent(PathBuf::new())
        );
        // `ParentDir`
        assert_eq!(
            FilePath::new("foo/..").err().unwrap(),
            FilePathError::InvalidPathComponent(PathBuf::from("foo"))
        );

        // `CurDir`
        assert_eq!(
            FilePath::new("./foo\\baz").err().unwrap(),
            FilePathError::InvalidPathComponent(PathBuf::new())
        );
        // But this works:
        let foobaz = FilePath::new("foo\\.\\baz").unwrap();
        assert_eq!(foobaz.to_owned(), FilePathBuf::new("foo/baz").unwrap());
    }

    #[test]
    #[allow(non_snake_case)]
    fn EmptyPathComponent() {
        // Repeated path separators are ignored and thus do not generate an empty path component.
        let foobaz = FilePath::new("foo\\\\baz").unwrap();
        assert_eq!(foobaz.to_owned(), FilePathBuf::new("foo/baz").unwrap());

        let foobaz = FilePath::new("foo//baz").unwrap();
        assert_eq!(foobaz.to_owned(), FilePathBuf::new("foo/baz").unwrap());
    }

    #[test]
    fn components() {
        // `.` in the middle is ignored.
        // Repeated path separators are ignored.
        // Trailing path separators are ignored.
        let path = FilePath::new("foo/./bar//Baz\\\\BILL\\").unwrap();
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
        let l = FilePath::new("foo/./bar//Baz\\\\BILL\\").unwrap();
        let r = FilePath::new("foo/bar/Baz/BILL").unwrap();
        assert_eq!(l, r);
        assert_eq!(l.as_path(), r.as_path());
        // Strings are different ...
        assert_ne!(l.as_str(), r.as_str());
        // ... but hashes are equal.
        let mut hl = std::collections::hash_map::DefaultHasher::new();
        let mut hr = hl.clone();
        l.hash(&mut hl);
        r.hash(&mut hr);
        assert_eq!(hl.finish(), hr.finish());
    }
}
