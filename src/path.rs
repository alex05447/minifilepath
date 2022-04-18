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
        Self::validate_filepath(path.as_ref())?;
        // We validated it, so it's safe to convert the path directly to a (non-empty) UTF-8 string slice.
        Ok(unsafe { Self::from_path(path.as_ref()) })
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

    /// Returns the length in bytes of the [`FilePath`]. Always > 0.
    pub fn len(&self) -> usize {
        self.0.len()
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
    pub fn components(&self) -> impl DoubleEndedIterator<Item = FilePathComponent<'_>> {
        // Need to use `PathIter` instead of `FilePathIter` because of `std::path::Path` quirks, see the comments for `FilePath`.
        PathIter::new(self)
    }

    /// Returns the file name portion of the [`FilePath`] (i.e. the last/leaf component).
    ///
    /// E.g.
    /// ```
    /// use {minifilepath::FilePath, ministr_macro::nestr};
    ///
    /// assert_eq!(FilePath::new("foo/bar.txt").unwrap().file_name(), nestr!("bar.txt"));
    /// assert_eq!(FilePath::new("foo/.txt").unwrap().file_name(), nestr!(".txt"));
    /// assert_eq!(FilePath::new("foo/bar/baz").unwrap().file_name(), nestr!("baz"));
    /// ```
    pub fn file_name(&self) -> FilePathComponent<'_> {
        match self.components().next_back() {
            Some(file_name) => file_name,
            None => debug_unreachable("empty `FilePath`'s are invalid"),
        }
    }

    /// Returns the file stem portion of the [`FilePath`] (i.e. the non-extension part of the last/leaf component).
    ///
    /// NOTE: this differs from standard library behaviour. Also see [`file_name_and_extension`].
    ///
    /// E.g.
    /// ```
    /// use {minifilepath::FilePath, ministr_macro::nestr};
    ///
    /// assert_eq!(FilePath::new("foo/bar.txt").unwrap().file_stem(), Some(nestr!("bar")));
    /// assert_eq!(FilePath::new("foo/.txt").unwrap().file_stem(), None);
    /// assert_eq!(FilePath::new("foo/bar/baz").unwrap().file_stem(), Some(nestr!("baz")));
    /// ```
    pub fn file_stem(&self) -> Option<FilePathComponent<'_>> {
        let file_name = self.file_name();
        file_stem_and_extension(file_name)
            .map(|file_stem_and_extension| file_stem_and_extension.file_stem)
            .unwrap_or(Some(file_name))
    }

    /// Returns the file stem portion of the [`FilePath`] (i.e. the non-extension part of the last/leaf component).
    ///
    /// NOTE: this differs from standard library behaviour. Also see [`file_name_and_extension`].
    ///
    /// E.g.
    /// ```
    /// use {minifilepath::FilePath, ministr_macro::nestr};
    ///
    /// assert_eq!(FilePath::new("foo/bar.txt").unwrap().extension(), Some(nestr!("txt")));
    /// assert_eq!(FilePath::new("foo/.txt").unwrap().extension(), Some(nestr!("txt")));
    /// assert_eq!(FilePath::new("foo/bar/baz").unwrap().extension(), None);
    /// ```
    pub fn extension(&self) -> Option<FilePathComponent<'_>> {
        file_stem_and_extension(self.file_name())
            .map(|file_name_and_extension| file_name_and_extension.extension)
    }

    /// The caller guarantees `path` is a valid file path.
    /// In this case it is safe to directly convert a `NonEmptyStr` to a `FilePath`.
    pub(crate) unsafe fn from_str(path: &NonEmptyStr) -> &Self {
        &*(path.as_str() as *const str as *const FilePath)
    }

    /// The caller guarantees `path` is a valid non-empty UTF-8 string slice and a valid file path.
    /// In this case it is safe to directly convert a non-empty UTF-8 `OsStr` to a `FilePath`.
    pub(crate) unsafe fn from_path(path: &Path) -> &Self {
        debug_assert!(!path.as_os_str().is_empty());
        &*(path.as_os_str() as *const OsStr as *const str as *const FilePath)
    }

    fn validate_filepath(path: &Path) -> Result<(), FilePathError> {
        if iterate_path(path)? {
            Ok(())
        } else {
            Err(FilePathError::EmptyPath)
        }
    }

    fn is_valid_filepath(path: &Path) -> bool {
        match iterate_path(path) {
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

        for component in self.components() {
            if !string.is_empty() {
                string.push(SEPARATOR_CHAR);
            }
            string.push_str(component);
        }

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
    use {super::*, std::path::PathBuf};

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
    fn RootDirectory() {
        assert_eq!(
            FilePath::new("/foo").err().unwrap(),
            FilePathError::RootDirectory
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn CurrentDirectory() {
        assert_eq!(
            FilePath::new("./foo\\baz").err().unwrap(),
            FilePathError::CurrentDirectory(PathBuf::new())
        );
        // But this works:
        let foobaz = FilePath::new("foo\\.\\baz").unwrap();
        assert_eq!(foobaz.to_owned(), FilePathBuf::new("foo/baz").unwrap());
    }

    #[test]
    #[allow(non_snake_case)]
    fn ParentDirectory() {
        assert_eq!(
            FilePath::new("..\\foo").err().unwrap(),
            FilePathError::ParentDirectory(PathBuf::new())
        );
        assert_eq!(
            FilePath::new("foo/..").err().unwrap(),
            FilePathError::ParentDirectory(PathBuf::from("foo"))
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn EmptyComponent() {
        // Repeated path separators are ignored and thus do not generate an empty path component.
        let foobaz = FilePath::new("foo\\\\baz").unwrap();
        assert_eq!(foobaz.to_owned(), FilePathBuf::new("foo/baz").unwrap());

        let foobaz = FilePath::new("foo//baz").unwrap();
        assert_eq!(foobaz.to_owned(), FilePathBuf::new("foo/baz").unwrap());
    }

    #[test]
    #[allow(non_snake_case)]
    fn ComponentTooLong() {
        let invalid_len = MAX_COMPONENT_LEN + 1;
        let invalid_component = vec![b'a'; invalid_len];
        let invalid_component = unsafe { std::str::from_utf8_unchecked(&invalid_component) };

        assert_eq!(
            FilePath::new(invalid_component).err().unwrap(),
            FilePathError::ComponentTooLong((PathBuf::from(invalid_component), invalid_len))
        );

        let valid_component = vec![b'a'; MAX_COMPONENT_LEN];
        let valid_component = unsafe { std::str::from_utf8_unchecked(&valid_component) };
        FilePath::new(valid_component).unwrap();
    }

    #[test]
    #[allow(non_snake_case)]
    fn InvalidCharacter() {
        assert_eq!(
            FilePath::new("foo\\a?").err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::from("foo\\a?"), '?'))
        );
        assert_eq!(
            FilePath::new("foo/BAR/*").err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::from("foo/BAR/*"), '*'))
        );
        assert_eq!(
            FilePath::new("foo/bar<1>").err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::from("foo/bar<1>"), '<'))
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn ComponentEndsWithAPeriod() {
        assert_eq!(
            FilePath::new("foo\\...").err().unwrap(),
            FilePathError::ComponentEndsWithAPeriod(PathBuf::from("foo\\..."))
        );
        // But this is a parent directory.
        assert_eq!(
            FilePath::new("foo\\..").err().unwrap(),
            FilePathError::ParentDirectory(PathBuf::from("foo"))
        );
        // And this is a current directory.
        assert_eq!(
            FilePath::new("./foo").err().unwrap(),
            FilePathError::CurrentDirectory(PathBuf::new())
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn ComponentEndsWithASpace() {
        assert_eq!(
            FilePath::new("foo\\bar.txt ").err().unwrap(),
            FilePathError::ComponentEndsWithASpace(PathBuf::from("foo\\bar.txt "))
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn ReservedName() {
        assert_eq!(
            FilePath::new("foo\\NUL").err().unwrap(),
            FilePathError::ReservedName(PathBuf::from("foo\\NUL"))
        );
        assert_eq!(
            FilePath::new("BAR/com7").err().unwrap(),
            FilePathError::ReservedName(PathBuf::from("BAR/com7"))
        );
    }

    #[cfg(windows)]
    #[test]
    #[allow(non_snake_case)]
    fn InvalidUTF8() {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt};

        // "foo/b<?>r"
        let wchars = [0x0066, 0x006f, 0x006f, 0x002f, 0x0062, 0xD800, 0x0072];
        let os_string = OsString::from_wide(&wchars[..]);
        let os_str = os_string.as_os_str();
        assert_eq!(os_str.to_string_lossy(), "foo/b�r");

        assert_eq!(
            FilePath::new(os_str).err().unwrap(),
            FilePathError::InvalidUTF8(PathBuf::from("foo"))
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn EmptyPath() {
        assert_eq!(FilePath::new("").err().unwrap(), FilePathError::EmptyPath);
    }

    #[test]
    #[allow(non_snake_case)]
    fn PathTooLong() {
        let path_piece = "a/";
        // Trailing `/` is not counted, so need to add one extra to overflow.
        let num_path_pieces = MAX_PATH_LEN / path_piece.len() + 1;

        let mut valid_path: String = (0..num_path_pieces).map(|_| path_piece).collect();
        assert_eq!(valid_path.len(), MAX_PATH_LEN + 1);

        FilePath::new(&valid_path).unwrap();

        let invalid_path = {
            valid_path.push_str(path_piece);
            valid_path
        };
        assert_eq!(invalid_path.len(), MAX_PATH_LEN + 3);

        assert_eq!(
            FilePath::new(&invalid_path).err().unwrap(),
            FilePathError::PathTooLong(MAX_PATH_LEN + 2)
        );
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
