//! Some simple Rust wrapper types for non-empty, relative, case agnostic UTF-8 file system paths.

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

pub type FilePathComponent<'a> = &'a NonEmptyStr;

/// Attempts to validate the file path `component`.
///
/// Does not handle prefixed paths correctly, instead simply returning an [`InvalidCharacter`](enum.FilePathError.html#variant.InvalidCharacter) error.
pub fn is_valid_path_component(component: FilePathComponent) -> Result<(), FilePathError> {
    validate_path_component(component, || PathBuf::new())
}

fn validate_path_component<F: FnOnce() -> PathBuf>(
    component: FilePathComponent,
    f: F,
) -> Result<(), FilePathError> {
    // \/:*?"<>|
    // .
    // ..

    use FilePathError::*;

    if component == "." {
        Err(CurrentDirectory(f()))
    } else if component == ".." {
        Err(ParentDirectory(f()))
    } else if component.starts_with('/') {
        Err(RootDirectory)
    } else {
        validate_normal_path_component(component, f)
    }
}

fn validate_normal_path_component<F: FnOnce() -> PathBuf>(
    component: FilePathComponent,
    f: F,
) -> Result<(), FilePathError> {
    for c in component.chars() {
        match c {
            '\\' | '/' | ':' | '*' | '?' | '\"' | '<' | '>' | '|' => {
                return Err(FilePathError::InvalidCharacter((f(), c)))
            }
            _ => {}
        }
    }

    Ok(())
}

pub(crate) const SEPARATOR_CHAR: char = '/';

/// Returns `true` if the path is not empty.
pub(crate) fn iterate_path<P: AsRef<Path>, F: FnMut(FilePathComponent)>(
    path: P,
    mut f: F,
) -> Result<bool, FilePathError> {
    use FilePathError::*;

    let path = path.as_ref();

    let mut num_parts = 0;

    for (idx, comp) in path.components().enumerate() {
        let get_path = || path.components().take(idx).collect::<PathBuf>();
        match comp {
            Component::Normal(comp) => {
                if let Some(comp) = comp.to_str() {
                    let comp = NonEmptyStr::new(comp).ok_or_else(|| EmptyComponent(get_path()))?;

                    validate_normal_path_component(comp, get_path)?;

                    f(comp);

                    num_parts += 1;
                } else {
                    return Err(InvalidUTF8(get_path()));
                }
            }
            Component::Prefix(_) => return Err(PrefixedPath),
            Component::CurDir => return Err(CurrentDirectory(get_path())),
            Component::ParentDir => return Err(ParentDirectory(get_path())),
            Component::RootDir => return Err(RootDirectory),
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

#[cfg(test)]
mod tests {
    use {super::*, ministr_macro::nestr};

    #[test]
    fn invalid_path_component() {
        use FilePathError::*;

        assert_eq!(
            validate_path_component(nestr!("."), || { PathBuf::new() })
                .err()
                .unwrap(),
            CurrentDirectory(PathBuf::new())
        );
        assert_eq!(
            validate_path_component(nestr!(".."), || { PathBuf::new() })
                .err()
                .unwrap(),
            ParentDirectory(PathBuf::new())
        );

        // But this works.
        assert!(validate_path_component(nestr!(".txt"), || { PathBuf::new() }).is_ok());
        assert!(validate_path_component(nestr!("txt."), || { PathBuf::new() }).is_ok());
        assert!(validate_path_component(nestr!(".t.x.t."), || { PathBuf::new() }).is_ok());
        assert!(validate_path_component(nestr!("..txt"), || { PathBuf::new() }).is_ok());
        assert!(validate_path_component(nestr!("txt.."), || { PathBuf::new() }).is_ok());

        assert_eq!(
            validate_path_component(nestr!("/foo"), || { PathBuf::new() })
                .err()
                .unwrap(),
            RootDirectory
        );
        assert_eq!(
            validate_path_component(nestr!("/fo\\o"), || { PathBuf::new() })
                .err()
                .unwrap(),
            RootDirectory
        );
        assert_eq!(
            validate_path_component(nestr!("f/oo"), || { PathBuf::new() })
                .err()
                .unwrap(),
            InvalidCharacter((PathBuf::new(), '/'))
        );
        assert_eq!(
            validate_path_component(nestr!("foo\\"), || { PathBuf::new() })
                .err()
                .unwrap(),
            InvalidCharacter((PathBuf::new(), '\\'))
        );

        assert_eq!(
            validate_path_component(nestr!("C:foo"), || { PathBuf::new() })
                .err()
                .unwrap(),
            InvalidCharacter((PathBuf::new(), ':'))
        );
        assert_eq!(
            validate_path_component(nestr!(":foo"), || { PathBuf::new() })
                .err()
                .unwrap(),
            InvalidCharacter((PathBuf::new(), ':'))
        );

        assert_eq!(
            validate_path_component(nestr!("\"foo\""), || { PathBuf::new() })
                .err()
                .unwrap(),
            InvalidCharacter((PathBuf::new(), '\"'))
        );

        assert_eq!(
            validate_path_component(nestr!("foo?"), || { PathBuf::new() })
                .err()
                .unwrap(),
            InvalidCharacter((PathBuf::new(), '?'))
        );

        assert_eq!(
            validate_path_component(nestr!("f*oo"), || { PathBuf::new() })
                .err()
                .unwrap(),
            InvalidCharacter((PathBuf::new(), '*'))
        );

        assert_eq!(
            validate_path_component(nestr!("foo<"), || { PathBuf::new() })
                .err()
                .unwrap(),
            InvalidCharacter((PathBuf::new(), '<'))
        );
        assert_eq!(
            validate_path_component(nestr!("foo>"), || { PathBuf::new() })
                .err()
                .unwrap(),
            InvalidCharacter((PathBuf::new(), '>'))
        );

        assert_eq!(
            validate_path_component(nestr!("foo|"), || { PathBuf::new() })
                .err()
                .unwrap(),
            InvalidCharacter((PathBuf::new(), '|'))
        );

        // But this works.
        assert!(validate_path_component(nestr!("foo"), || { PathBuf::new() }).is_ok());
        assert!(validate_path_component(nestr!("βαρ"), || { PathBuf::new() }).is_ok());
    }
}
