//! Some simple wrapper types for non-empty, relative, case agnostic UTF-8 file system paths.

mod builder;
mod error;
mod iter;
mod path;
mod pathbuf;

pub(crate) use iter::*;
pub use {builder::*, error::*, path::*, pathbuf::*};

use {
    ministr::NonEmptyStr,
    std::path::{Component, Path, PathBuf},
};

pub(crate) const SEPARATOR_CHAR: char = '/';

/// Returns the number of valid file path components.
pub(crate) fn iterate_path<P: AsRef<Path>, F: FnMut(&NonEmptyStr)>(
    path: P,
    mut f: F,
) -> Result<bool, FilePathError> {
    use FilePathError::*;

    let path = path.as_ref();

    let mut num_parts = 0;

    for (idx, comp) in path.components().enumerate() {
        match comp {
            Component::Normal(comp) => {
                if let Some(comp) = comp.to_str() {
                    let comp = NonEmptyStr::new(comp).ok_or_else(|| {
                        EmptyPathComponent(path.components().take(idx).collect::<PathBuf>())
                    })?;

                    f(comp);

                    num_parts += 1;
                } else {
                    return Err(InvalidUTF8(
                        path.components().take(idx).collect::<PathBuf>(),
                    ));
                }
            }
            Component::Prefix(_) => return Err(PrefixedPath),
            Component::CurDir | Component::ParentDir | Component::RootDir => {
                return Err(InvalidPathComponent(
                    path.components().take(idx).collect::<PathBuf>(),
                ))
            }
        }
    }

    Ok(num_parts > 0)
}

pub(crate) fn debug_unreachable(msg: &'static str) -> ! {
    if cfg!(debug_assertions) {
        unreachable!(msg)
    } else {
        unsafe { std::hint::unreachable_unchecked() }
    }
}
