use {crate::*, ministr::NonEmptyString, std::path::Path};

/// Builder for a [`FilePathBuf`].
///
/// Allows constructing valid [`FilePathBuf`]'s and reusing the heap-allocated buffer if necessary.
#[derive(Clone, Debug)]
pub struct FilePathBuilder(String);

impl FilePathBuilder {
    /// Creates an empty [`FilePathBuilder`].
    pub fn new() -> Self {
        Self(String::new())
    }

    /// Creates an empty [`FilePathBuilder`] with `capacity` bytes reserved.
    pub fn with_capacity(capacity: usize) -> Self {
        Self(String::with_capacity(capacity))
    }

    /// Returns the length in bytes of the built [`FilePathBuf`].
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Attempts to append the `path` to the built [`FilePathBuf`].
    ///
    /// Returns an [`error`](FilePathError) if the `path` contains an invalid component.
    pub fn push<P: AsRef<Path>>(&mut self, path: P) -> Result<(), FilePathError> {
        FilePath::new(path.as_ref()).map(|path| append_file_path_to_string(path, &mut self.0))
    }

    /// Attempts to pop the last (leaf) path component of the built [`FilePathBuf`].
    ///
    /// Returns `true` if the built [`FilePathBuf`] was not empty and the last path component was popped.
    pub fn pop(&mut self) -> bool {
        let res = !self.is_empty();
        while let Some(c) = self.0.pop() {
            if c == SEPARATOR_CHAR {
                debug_assert!(!self.0.is_empty());
                return res;
            }
        }
        debug_assert!(self.0.is_empty());
        res
    }

    /// Clears the built [`FilePathBuf`], without reducing its capacity.
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Consumes the [`FilePathBuilder`] and, if it is non-empty, returns the built [`FilePathBuf`].
    pub fn build(self) -> Option<FilePathBuf> {
        NonEmptyString::new(self.0).map(FilePathBuf)
    }

    pub fn into_inner(self) -> String {
        self.0
    }

    pub(crate) fn from(mut buf: String) -> Self {
        buf.clear();
        Self(buf)
    }

    #[cfg(test)]
    fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

pub(crate) fn append_file_path_to_string(path: &FilePath, string: &mut String) {
    for component in path.components() {
        append_path_component_to_string(component, string)
    }
}

fn append_path_component_to_string(component: FilePathComponent, string: &mut String) {
    if !string.is_empty() {
        string.push(SEPARATOR_CHAR);
    }
    string.push_str(component);
}

#[cfg(test)]
mod tests {
    use {super::*, std::path::PathBuf};

    #[test]
    fn builder() {
        let mut builder = FilePathBuilder::new();
        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
        assert_eq!(builder.as_str(), "");

        assert_eq!(
            builder.push("C:/foo").err().unwrap(),
            FilePathError::PrefixedPath
        );
        assert_eq!(
            builder.push("/foo").err().unwrap(),
            FilePathError::RootDirectory
        );
        assert_eq!(
            builder.push("foo/../").err().unwrap(),
            FilePathError::ParentDirectory(PathBuf::from("foo"))
        );
        assert_eq!(
            builder.push("./foo").err().unwrap(),
            FilePathError::CurrentDirectory(PathBuf::new())
        );

        builder.push("foo/./").unwrap();
        assert!(!builder.is_empty());
        assert_eq!(builder.as_str(), "foo");

        builder.push("Bar\\\\").unwrap();
        assert!(!builder.is_empty());
        assert_eq!(builder.as_str(), "foo/Bar");

        builder.push("baz/./BILL//").unwrap();
        assert!(!builder.is_empty());
        assert_eq!(builder.as_str(), "foo/Bar/baz/BILL");

        assert!(builder.pop());
        assert!(!builder.is_empty());
        assert_eq!(builder.as_str(), "foo/Bar/baz");

        let path = builder.build().unwrap();
        assert_eq!(path.as_str(), "foo/Bar/baz");

        let builder = path.into_builder();
        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
        assert_eq!(builder.as_str(), "");
    }
}
