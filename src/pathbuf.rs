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
            "tried to create a `FilePathBuf` from an invalid path `String`"
        );
        Self(unsafe { NonEmptyString::new_unchecked(path) })
    }

    /// Returns the length in bytes of the [`FilePathBuf`]. Always > 0.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Converts the [`FilePathBuf`] back to a [`FilePathBuilder`], without clearing it,
    /// allowing the buffer and the built path to be reused.
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

    /// Returns an [`iterator`](FilePathBufIter) over the (non-empty, UTF-8 string) components of the [`FilePathBuf`], root to leaf.
    ///
    /// NOTE: file name, with extension or not, is a single component.
    ///
    /// NOTE: can be reversed via `rev()` to iterate leaf to root.
    pub fn components(&self) -> FilePathBufIter<'_> {
        // Unlike `FilePath`, we may use the simpler iterator because of the `FilePathBuf`'s canonical string representation.
        FilePathBufIter::new(self.as_file_path())
    }

    /// Returns the file name portion of the [`FilePathBuf`] (i.e. the last/leaf component).
    ///
    /// E.g.
    /// ```
    /// use {minifilepath::FilePathBuf, ministr_macro::nestr};
    ///
    /// assert_eq!(FilePathBuf::new("foo/bar.txt").unwrap().file_name(), nestr!("bar.txt"));
    /// assert_eq!(FilePathBuf::new("foo/.txt").unwrap().file_name(), nestr!(".txt"));
    /// assert_eq!(FilePathBuf::new("foo/bar/baz").unwrap().file_name(), nestr!("baz"));
    /// ```
    pub fn file_name(&self) -> FilePathComponent<'_> {
        unsafe {
            self.components()
                .next_back()
                .unwrap_unchecked_dbg_msg("empty `FilePathBuf`'s are invalid")
        }
    }

    /// Returns the file stem portion of the [`FilePathBuf`] (i.e. the non-extension part of the last/leaf component).
    ///
    /// NOTE: this differs from standard library behaviour. Also see [`file_stem_and_extension()`].
    ///
    /// E.g.
    /// ```
    /// use {minifilepath::FilePathBuf, ministr_macro::nestr};
    ///
    /// assert_eq!(FilePathBuf::new("foo/bar.txt").unwrap().file_stem(), Some(nestr!("bar")));
    /// assert_eq!(FilePathBuf::new("foo/.txt").unwrap().file_stem(), None);
    /// assert_eq!(FilePathBuf::new("foo/bar/baz").unwrap().file_stem(), Some(nestr!("baz")));
    /// ```
    pub fn file_stem(&self) -> Option<FilePathComponent<'_>> {
        let file_name = self.file_name();
        file_stem_and_extension(file_name)
            .map(|file_stem_and_extension| file_stem_and_extension.file_stem)
            .unwrap_or(Some(file_name))
    }

    /// Returns the extension portion of the [`FilePathBuf`] (i.e. the extension part of the last/leaf component).
    ///
    /// NOTE: this differs from standard library behaviour. Also see [`file_stem_and_extension()`].
    ///
    /// E.g.
    /// ```
    /// use {minifilepath::FilePathBuf, ministr_macro::nestr};
    ///
    /// assert_eq!(FilePathBuf::new("foo/bar.txt").unwrap().extension(), Some(nestr!("txt")));
    /// assert_eq!(FilePathBuf::new("foo/.txt").unwrap().extension(), Some(nestr!("txt")));
    /// assert_eq!(FilePathBuf::new("foo/bar/baz").unwrap().extension(), None);
    /// ```
    pub fn extension(&self) -> Option<FilePathComponent<'_>> {
        file_stem_and_extension(self.file_name())
            .map(|file_stem_and_extension| file_stem_and_extension.extension)
    }

    /// Used to debug validate the `path` in `new_unchecked()`.
    #[cfg(debug_assertions)]
    fn is_valid_filepath(path: &str) -> bool {
        // `path` is a valid `FilePathBuf` if a `FilePathBuf` created from it has the same string representation.
        Self::new(path).is_ok_and(|path_| path_.as_str() == path)
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

impl From<&FilePath> for FilePathBuf {
    fn from(path: &FilePath) -> Self {
        path.to_owned()
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
    fn EmptyComponent() {
        // Repeated path separators are ignored and thus do not generate an empty path component.
        let foobaz = FilePathBuf::new("foo\\\\baz").unwrap();
        assert_eq!(foobaz.to_owned(), FilePathBuf::new("foo/baz").unwrap());

        let foobaz = FilePathBuf::new("foo//baz").unwrap();
        assert_eq!(foobaz.to_owned(), FilePathBuf::new("foo/baz").unwrap());
    }

    #[test]
    #[allow(non_snake_case)]
    fn ComponentTooLong() {
        let invalid_len = MAX_COMPONENT_LEN + 1;
        let invalid_component = vec![b'a'; invalid_len];
        let invalid_component = unsafe { std::str::from_utf8_unchecked(&invalid_component) };

        assert_eq!(
            FilePathBuf::new(invalid_component).err().unwrap(),
            FilePathError::ComponentTooLong((PathBuf::from(invalid_component), invalid_len))
        );

        let valid_component = vec![b'a'; MAX_COMPONENT_LEN];
        let valid_component = unsafe { std::str::from_utf8_unchecked(&valid_component) };
        FilePathBuf::new(valid_component).unwrap();
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
            FilePathBuf::new(os_str).err().unwrap(),
            FilePathError::InvalidUTF8(PathBuf::from("foo"))
        );
    }

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
    fn PathTooLong() {
        let path_piece = "a/";
        // Trailing `/` is not counted, so need to add one extra to overflow.
        let num_path_pieces = MAX_PATH_LEN / path_piece.len() + 1;

        let mut valid_path: String = (0..num_path_pieces).map(|_| path_piece).collect();
        assert_eq!(valid_path.len(), MAX_PATH_LEN + 1);

        {
            let mut valid_path = FilePathBuf::new(&valid_path).unwrap().into_builder();
            assert_eq!(valid_path.len(), MAX_PATH_LEN);
            assert_eq!(
                valid_path.push(path_piece).err().unwrap(),
                FilePathError::PathTooLong(MAX_PATH_LEN + 2)
            );
        }

        let invalid_path = {
            valid_path.push_str(path_piece);
            valid_path
        };
        assert_eq!(invalid_path.len(), MAX_PATH_LEN + 3);

        assert_eq!(
            FilePathBuf::new(&invalid_path).err().unwrap(),
            FilePathError::PathTooLong(MAX_PATH_LEN + 2)
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
