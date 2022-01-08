use {
    crate::*,
    ministr::NonEmptyStr,
    std::{
        iter::{DoubleEndedIterator, Iterator},
        path::{Component, Components, Path},
    },
};

/// This is a lightweight iterator over the canonical path string using string splitting.
pub(crate) struct FilePathIter<'a>(Option<&'a FilePath>);

impl<'a> FilePathIter<'a> {
    /// The caller guarantees `path` is canonical (i.e. borrowed from a [`FilePathBuf`]).
    pub(crate) fn new(path: &'a FilePath) -> Self {
        Self(Some(path))
    }
}

impl<'a> Iterator for FilePathIter<'a> {
    type Item = FilePathComponent<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        next_impl(&mut self.0, pop_path_component_front)
    }
}

impl<'a> DoubleEndedIterator for FilePathIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        next_impl(&mut self.0, pop_path_component_back)
    }
}

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
pub(crate) fn pop_path_component_front(path: &FilePath) -> (FilePathComponent, Option<&FilePath>) {
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
pub(crate) fn pop_path_component_back(path: &FilePath) -> (FilePathComponent, Option<&FilePath>) {
    if let Some((path, comp)) = path.as_str().rsplit_once(SEPARATOR_CHAR) {
        (
            unsafe { NonEmptyStr::new_unchecked(comp) },
            NonEmptyStr::new(path).map(|path| unsafe { FilePath::from_str(path) }),
        )
    } else {
        (&path.0, None)
    }
}

/// This is a full, heavyweight iterator over the (potentially non-canonical) path using `std::path::Components`.
///
/// Used to iterate over [`FilePath`]'s, because those may be constructed from [`std::path::Path`]'s and might
/// 1) contain `CurDir` components (`.`),
/// 2) contain repeated path component separators,
/// 3) use different path component separators depending on the OS.
pub(crate) struct PathIter<'a>(pub(crate) Components<'a>);

impl<'a> PathIter<'a> {
    pub(crate) fn new(src: &'a FilePath) -> Self {
        Self(Path::new(src.as_str()).components())
    }
}

impl<'a> Iterator for PathIter<'a> {
    type Item = FilePathComponent<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(get_component)
    }
}

impl<'a> DoubleEndedIterator for PathIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(get_component)
    }
}

fn get_component<'a>(component: Component<'a>) -> FilePathComponent<'a> {
    match component {
        Component::Normal(component) => match component.to_str() {
            // Must succeed - `FilePath`'s only contain valid (non-empty) path components
            Some(str) => unsafe { NonEmptyStr::new_unchecked(str) },
            // Must succeed - `FilePath`'s only contain valid (UTF-8) path components.
            None => {
                debug_unreachable("`FilePath`'s must only contain valid (UTF-8) path components")
            }
        },
        // Must succeed - `FilePath`'s only contain valid (normal) path components.
        _ => debug_unreachable("`FilePath`'s must only contain valid (normal) path components"),
    }
}
