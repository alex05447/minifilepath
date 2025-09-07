use {
    crate::*,
    ministr::NonEmptyStr,
    miniunchecked::*,
    std::{
        iter::{DoubleEndedIterator, FusedIterator, Iterator},
        path::{Component, Components, Path},
    },
};

/// Lightweight double-ended iterator over the canonical [`path string`](FilePathBuf) using string splitting.
pub struct FilePathBufIter<'a>(Option<&'a FilePath>);

impl<'a> FilePathBufIter<'a> {
    /// The caller guarantees `path` is canonical (i.e. borrowed from a [`FilePathBuf`]).
    pub(crate) fn new(path: &'a FilePath) -> Self {
        Self(Some(path))
    }
}

impl<'a> Iterator for FilePathBufIter<'a> {
    type Item = FilePathComponent<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        next_impl(&mut self.0, pop_path_component_front)
    }
}

impl<'a> DoubleEndedIterator for FilePathBufIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        next_impl(&mut self.0, pop_path_component_back)
    }
}

impl<'a> FusedIterator for FilePathBufIter<'a> {}

fn next_impl<'a>(
    src_path: &mut Option<&'a FilePath>,
    pop: fn(&FilePath) -> (FilePathComponent, Option<&FilePath>),
) -> Option<FilePathComponent<'a>> {
    src_path.map(|path| {
        let (comp, path) = pop(path);
        *src_path = path;
        comp
    })
}

/// The caller guarantees `path` is a canonical `FilePath`.
pub(crate) fn pop_path_component_front(
    path: &FilePath,
) -> (FilePathComponent<'_>, Option<&FilePath>) {
    if let Some((comp, path)) = path.as_str().split_once(SEPARATOR_CHAR) {
        (
            unsafe { NonEmptyStr::new_unchecked(comp) },
            NonEmptyStr::new(path).map(|path| unsafe { FilePath::from_str(path) }),
        )
    } else {
        (&path.0, None)
    }
}

/// The caller guarantees `path` is a canonical `FilePath`.
pub(crate) fn pop_path_component_back(
    path: &FilePath,
) -> (FilePathComponent<'_>, Option<&FilePath>) {
    if let Some((path, comp)) = path.as_str().rsplit_once(SEPARATOR_CHAR) {
        (
            unsafe { NonEmptyStr::new_unchecked(comp) },
            NonEmptyStr::new(path).map(|path| unsafe { FilePath::from_str(path) }),
        )
    } else {
        (&path.0, None)
    }
}

/// This is a full, heavyweight double-ended iterator over the (potentially non-canonical) path using [`std::path::Components`].
///
/// Used to iterate over [`FilePath`]'s, because those may be constructed from [`std::path::Path`]'s and might
/// 1) contain `CurDir` components (`.`),
/// 2) contain repeated path component separators,
/// 3) use different path component separators depending on the OS.
pub struct FilePathIter<'a>(pub(crate) Components<'a>);

impl<'a> FilePathIter<'a> {
    pub(crate) fn new(src: &'a FilePath) -> Self {
        Self(Path::new(src.as_str()).components())
    }
}

impl<'a> Iterator for FilePathIter<'a> {
    type Item = FilePathComponent<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(get_component)
    }
}

impl<'a> DoubleEndedIterator for FilePathIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(get_component)
    }
}

impl<'a> FusedIterator for FilePathIter<'a> {}

fn get_component<'a>(component: Component<'a>) -> FilePathComponent<'a> {
    match component {
        // Must succeed - `FilePath`'s only contain valid (non-empty) path components
        Component::Normal(component) => unsafe {
            NonEmptyStr::new_unchecked(component.to_str().unwrap_unchecked_dbg_msg(
                "`FilePath`'s must only contain valid (UTF-8) path components",
            ))
        },
        // Must succeed - `FilePath`'s only contain valid (normal) path components.
        _ => unsafe {
            unreachable_dbg!("`FilePath`'s must only contain valid (normal) path components")
        },
    }
}
