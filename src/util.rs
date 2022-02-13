use {
    crate::*,
    ministr::NonEmptyStr,
    std::path::{Component, Path, PathBuf},
};

pub(crate) fn validate_normal_path_component<F: FnOnce() -> PathBuf>(
    component: FilePathComponent,
    f: F,
) -> Result<(), FilePathError> {
    let len = component.len();

    if len > MAX_COMPONENT_LEN {
        return Err(FilePathError::ComponentTooLong((f(), len)));
    }

    if component.ends_with('.') {
        return Err(FilePathError::ComponentEndsWithAPeriod(f()));
    }

    if component.ends_with(' ') {
        return Err(FilePathError::ComponentEndsWithASpace(f()));
    }

    let invalid_characters = ['\\', '/', ':', '*', '?', '\"', '<', '>', '|'];

    for c in component.chars() {
        if invalid_characters.contains(&c) {
            return Err(FilePathError::InvalidCharacter((f(), c)));
        }
    }

    if let Some((l, r)) = split_at_reserved_name(component) {
        let l = l.trim_end();
        let r = r.trim_start();

        // Reserved file names are not allowed, including the case with any extension.
        if l.is_empty() && (r.is_empty() || r.starts_with('.')) {
            return Err(FilePathError::ReservedName(f()));
        }
    }

    Ok(())
}

/// Like `str::split_once(...)`, but splits (case-insensitively) on one of the Windows reserved file names.
fn split_at_reserved_name(component: FilePathComponent) -> Option<(&str, &str)> {
    // None of the reserved name match sequences overlap, except `CON` / `COM?`, which diverge on their 3rd matched character,
    // which allows us to implement this efficiently by only ever tracking at most a single match sequence.

    // let reserved_names = [
    //     "AUX",
    //     "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    //     "CON",
    //     "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    //     "NUL",
    //     "PRN",
    // ];

    enum AcceptResult {
        /// Failed to match a char, reset, keep processing.
        NoMatch,
        /// Matched a char, match still incomplete, keep processing.
        Accepted,
        /// Matched a char, completed a match.
        /// Contains the offset in bytes to the start of the match; `3` for `COM?` / `LPT?`, `2` for the rest.
        AcceptedAndFinished(usize),
    }

    trait ReservedNameMatch {
        fn accept(&mut self, c: char) -> AcceptResult;
    }

    enum AUX {
        A,
        U,
    }

    impl ReservedNameMatch for AUX {
        fn accept(&mut self, c: char) -> AcceptResult {
            match self {
                Self::A => {
                    if c == 'u' {
                        *self = Self::U;
                        return AcceptResult::Accepted;
                    }
                }
                Self::U => {
                    if c == 'x' {
                        return AcceptResult::AcceptedAndFinished(2);
                    }
                }
            }

            AcceptResult::NoMatch
        }
    }

    enum NUL {
        N,
        U,
    }

    impl ReservedNameMatch for NUL {
        fn accept(&mut self, c: char) -> AcceptResult {
            match self {
                Self::N => {
                    if c == 'u' {
                        *self = Self::U;
                        return AcceptResult::Accepted;
                    }
                }
                Self::U => {
                    if c == 'l' {
                        return AcceptResult::AcceptedAndFinished(2);
                    }
                }
            }

            AcceptResult::NoMatch
        }
    }

    enum PRN {
        P,
        R,
    }

    impl ReservedNameMatch for PRN {
        fn accept(&mut self, c: char) -> AcceptResult {
            match self {
                Self::P => {
                    if c == 'r' {
                        *self = Self::R;
                        return AcceptResult::Accepted;
                    }
                }
                Self::R => {
                    if c == 'n' {
                        return AcceptResult::AcceptedAndFinished(2);
                    }
                }
            }

            AcceptResult::NoMatch
        }
    }

    enum CONOrM {
        C,
        O,
        M,
    }

    impl ReservedNameMatch for CONOrM {
        fn accept(&mut self, c: char) -> AcceptResult {
            match self {
                Self::C => {
                    if c == 'o' {
                        *self = Self::O;
                        return AcceptResult::Accepted;
                    }
                }
                Self::O => match c {
                    'n' => return AcceptResult::AcceptedAndFinished(2),
                    'm' => {
                        *self = Self::M;
                        return AcceptResult::Accepted;
                    }
                    _ => {}
                },
                Self::M => match c {
                    '1'..='9' => return AcceptResult::AcceptedAndFinished(3),
                    _ => {}
                },
            }

            AcceptResult::NoMatch
        }
    }

    enum LPT {
        L,
        P,
        T,
    }

    impl ReservedNameMatch for LPT {
        fn accept(&mut self, c: char) -> AcceptResult {
            match self {
                Self::L => {
                    if c == 'p' {
                        *self = Self::P;
                        return AcceptResult::Accepted;
                    }
                }
                Self::P => {
                    if c == 't' {
                        *self = Self::T;
                        return AcceptResult::Accepted;
                    }
                }
                Self::T => match c {
                    '1'..='9' => return AcceptResult::AcceptedAndFinished(3),
                    _ => {}
                },
            }

            AcceptResult::NoMatch
        }
    }

    enum ReservedName {
        AUX(AUX),
        NUL(NUL),
        PRN(PRN),
        CONOrM(CONOrM),
        LPT(LPT),
    }

    impl ReservedNameMatch for ReservedName {
        fn accept(&mut self, c: char) -> AcceptResult {
            match self {
                Self::AUX(aux) => aux.accept(c),
                Self::NUL(nul) => nul.accept(c),
                Self::PRN(prn) => prn.accept(c),
                Self::CONOrM(conorm) => conorm.accept(c),
                Self::LPT(lpt) => lpt.accept(c),
            }
        }
    }

    let restart = |c: char, r: &mut Option<ReservedName>| match c {
        'a' => {
            r.replace(ReservedName::AUX(AUX::A));
        }
        'n' => {
            r.replace(ReservedName::NUL(NUL::N));
        }
        'p' => {
            r.replace(ReservedName::PRN(PRN::P));
        }
        'c' => {
            r.replace(ReservedName::CONOrM(CONOrM::C));
        }
        'l' => {
            r.replace(ReservedName::LPT(LPT::L));
        }
        _ => {
            r.take();
        }
    };

    let mut reserved_name: Option<ReservedName> = None;

    for (idx, c) in component.char_indices() {
        // All reserved names are ASCII.
        if c.is_ascii() {
            // Case-insensitive.
            let c = c.to_ascii_lowercase();
            if let Some(reserved_name_) = reserved_name.as_mut() {
                match reserved_name_.accept(c) {
                    AcceptResult::NoMatch => {
                        restart(c, &mut reserved_name);
                    }
                    AcceptResult::Accepted => {}
                    AcceptResult::AcceptedAndFinished(offset) => {
                        debug_assert!(idx >= offset);
                        let l_end = idx - offset;
                        let l = unsafe { component.get_unchecked(..l_end) };
                        let r_start = idx + 1;
                        debug_assert!(r_start <= component.len());
                        let r = unsafe { component.get_unchecked(r_start..) };
                        return Some((l, r));
                    }
                }
            } else {
                restart(c, &mut reserved_name);
            }
        } else {
            reserved_name.take();
        }
    }

    None
}

/// Returns `true` if the path is not empty.
pub(crate) fn iterate_path<P: AsRef<Path>>(path: P) -> Result<bool, FilePathError> {
    use FilePathError::*;

    let path = path.as_ref();

    let mut path_len: usize = 0;

    let get_path = |idx: usize, include_self: bool| {
        path.components()
            .take(if include_self { idx + 1 } else { idx })
            .collect::<PathBuf>()
    };

    for (idx, comp) in path.components().enumerate() {
        match comp {
            Component::Normal(comp) => {
                if let Some(comp) = comp.to_str() {
                    let comp = NonEmptyStr::new(comp)
                        .ok_or_else(|| EmptyComponent(get_path(idx, false)))?;

                    validate_normal_path_component(comp, || get_path(idx, true))?;

                    if path_len != 0 {
                        path_len += 1;
                    }

                    path_len += comp.len();

                } else {
                    return Err(InvalidUTF8(get_path(idx, false)));
                }
            }
            Component::Prefix(_) => return Err(PrefixedPath),
            Component::CurDir => return Err(CurrentDirectory(get_path(idx, false))),
            Component::ParentDir => return Err(ParentDirectory(get_path(idx, false))),
            Component::RootDir => return Err(RootDirectory),
        }
    }

    if path_len > MAX_PATH_LEN {
        return Err(PathTooLong(path_len));
    }

    Ok(path_len > 0)
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
    fn split_at_reserved_name_() {
        assert!(split_at_reserved_name(nestr!("f")).is_none());
        assert!(split_at_reserved_name(nestr!("foo")).is_none());
        assert!(split_at_reserved_name(nestr!("comt")).is_none());

        assert_eq!(
            split_at_reserved_name(nestr!("fAuX.txt")).unwrap(),
            ("f", ".txt")
        );
        assert_eq!(
            split_at_reserved_name(nestr!(". PRnt")).unwrap(),
            (". ", "t")
        );
        assert_eq!(split_at_reserved_name(nestr!("NUL")).unwrap(), ("", ""));
        assert_eq!(split_at_reserved_name(nestr!("COM7.")).unwrap(), ("", "."));
        assert_eq!(split_at_reserved_name(nestr!("acon ")).unwrap(), ("a", " "));
    }

    fn validate_normal_path_component_(component: &NonEmptyStr) -> Result<(), FilePathError> {
        validate_normal_path_component(component, || PathBuf::new())
    }

    #[allow(non_snake_case)]
    #[test]
    fn InvalidCharacter() {
        assert_eq!(
            validate_normal_path_component_(nestr!("/foo"))
                .err()
                .unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '/'))
        );
        assert_eq!(
            validate_normal_path_component_(nestr!("f/oo"))
                .err()
                .unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '/'))
        );
        assert_eq!(
            validate_normal_path_component_(nestr!("foo\\"))
                .err()
                .unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '\\'))
        );

        assert_eq!(
            validate_normal_path_component_(nestr!("C:foo"))
                .err()
                .unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), ':'))
        );
        assert_eq!(
            validate_normal_path_component_(nestr!(":foo"))
                .err()
                .unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), ':'))
        );

        assert_eq!(
            validate_normal_path_component_(nestr!("\"foo\""))
                .err()
                .unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '\"'))
        );

        assert_eq!(
            validate_normal_path_component_(nestr!("foo?"))
                .err()
                .unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '?'))
        );

        assert_eq!(
            validate_normal_path_component_(nestr!("f*oo"))
                .err()
                .unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '*'))
        );

        assert_eq!(
            validate_normal_path_component_(nestr!("foo<"))
                .err()
                .unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '<'))
        );
        assert_eq!(
            validate_normal_path_component_(nestr!("foo>"))
                .err()
                .unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '>'))
        );

        assert_eq!(
            validate_normal_path_component_(nestr!("foo|"))
                .err()
                .unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '|'))
        );

        // But this works.
        assert!(validate_normal_path_component_(nestr!("foo")).is_ok());
        assert!(validate_normal_path_component_(nestr!("βαρ")).is_ok());
    }

    #[allow(non_snake_case)]
    #[test]
    fn ComponentEndsWithAPeriod() {
        assert_eq!(
            validate_normal_path_component_(nestr!("..."))
                .err()
                .unwrap(),
            FilePathError::ComponentEndsWithAPeriod(PathBuf::new())
        );
        assert_eq!(
            validate_normal_path_component_(nestr!("foo."))
                .err()
                .unwrap(),
            FilePathError::ComponentEndsWithAPeriod(PathBuf::new())
        );
        assert_eq!(
            validate_normal_path_component_(nestr!("NUL."))
                .err()
                .unwrap(),
            FilePathError::ComponentEndsWithAPeriod(PathBuf::new())
        );
    }

    #[allow(non_snake_case)]
    #[test]
    fn ComponentEndsWithASpace() {
        assert_eq!(
            validate_normal_path_component_(nestr!("foo "))
                .err()
                .unwrap(),
            FilePathError::ComponentEndsWithASpace(PathBuf::new())
        );
        assert_eq!(
            validate_normal_path_component_(nestr!("foo . "))
                .err()
                .unwrap(),
            FilePathError::ComponentEndsWithASpace(PathBuf::new())
        );
        assert_eq!(
            validate_normal_path_component_(nestr!("LPT7 "))
                .err()
                .unwrap(),
            FilePathError::ComponentEndsWithASpace(PathBuf::new())
        );

        // But this works.
        validate_normal_path_component_(nestr!("foo .txt")).unwrap();
    }

    #[allow(non_snake_case)]
    #[test]
    fn ReservedName() {
        assert_eq!(
            validate_normal_path_component_(nestr!("COM7"))
                .err()
                .unwrap(),
            FilePathError::ReservedName(PathBuf::new())
        );
        assert_eq!(
            validate_normal_path_component_(nestr!("CON"))
                .err()
                .unwrap(),
            FilePathError::ReservedName(PathBuf::new())
        );
        assert_eq!(
            validate_normal_path_component_(nestr!(" AUX"))
                .err()
                .unwrap(),
            FilePathError::ReservedName(PathBuf::new())
        );
        assert_eq!(
            validate_normal_path_component_(nestr!("NUL.txt"))
                .err()
                .unwrap(),
            FilePathError::ReservedName(PathBuf::new())
        );
        assert_eq!(
            validate_normal_path_component_(nestr!("LPT4 .txt.bmp"))
                .err()
                .unwrap(),
            FilePathError::ReservedName(PathBuf::new())
        );

        // But this works.
        validate_normal_path_component_(nestr!("faux")).unwrap();
        validate_normal_path_component_(nestr!("COM")).unwrap();
        validate_normal_path_component_(nestr!("COM71")).unwrap();
        validate_normal_path_component_(nestr!("lpt0")).unwrap();
        validate_normal_path_component_(nestr!(".NUL")).unwrap();
        validate_normal_path_component_(nestr!("foo.PRN")).unwrap();
    }
}
