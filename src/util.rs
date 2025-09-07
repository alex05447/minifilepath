use {
    crate::*,
    ministr::NonEmptyStr,
    std::path::{Component, Path, PathBuf},
};

pub(crate) fn validate_path_component<F: FnOnce() -> PathBuf>(
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
        if c.is_ascii_control() || invalid_characters.contains(&c) {
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
fn split_at_reserved_name(component: FilePathComponent<'_>) -> Option<(&str, &str)> {
    // None of the reserved name match sequences overlap, except `CON` / `COM?`, which diverge on their 3rd matched character,
    // which allows us to implement this efficiently by only ever tracking at most a single match sequence.

    // let reserved_names = [
    //     "AUX",
    //     "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "COM0",
    //     "CON",
    //     "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "LPT0",
    //     "NUL",
    //     "PRN",
    //     "CONIN$", "CONOUT$"
    // ];

    enum AcceptResult {
        /// Failed to match a char, reset, keep processing.
        NoMatch,
        /// Matched a char, match still incomplete, keep processing.
        Accepted,
        /// Matched a char, completed a match.
        /// Contains the tuple of
        /// - offset in bytes back from current character to the start of the match;
        ///   `2` for most, `3` for `COM?` / `LPT?`, `5` for `CONIN$`, `6` for `CONOUT$`;
        /// - offset in bytes back from the current character to the end of the match;
        ///   always `0` except when matching `CON?`, in which case it's `1` (to support also matching `CONIN$` / `CONOUT$`).
        AcceptedAndFinished((usize, usize)),
    }

    trait ReservedNameMatch
    where
        Self: Sized,
    {
        fn accept(&mut self, c: char) -> AcceptResult;

        /// Called when no match was found after having processed all characters.
        ///
        /// Handles the `CON?` case (to support also matching `CONIN$` / `CONOUT$`).
        fn finish(self) -> Option<(usize, usize)> {
            None
        }
    }

    #[allow(clippy::upper_case_acronyms)]
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
                        return AcceptResult::AcceptedAndFinished((2, 0));
                    }
                }
            }

            AcceptResult::NoMatch
        }
    }

    #[allow(clippy::upper_case_acronyms)]
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
                        return AcceptResult::AcceptedAndFinished((2, 0));
                    }
                }
            }

            AcceptResult::NoMatch
        }
    }

    #[allow(clippy::upper_case_acronyms)]
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
                        return AcceptResult::AcceptedAndFinished((2, 0));
                    }
                }
            }

            AcceptResult::NoMatch
        }
    }

    #[allow(clippy::upper_case_acronyms)]
    enum CONOrMOrINOrOUT {
        C,
        O,
        M,
        N,
        NI,
        NIN,
        NO,
        NOU,
        NOUT,
    }

    impl ReservedNameMatch for CONOrMOrINOrOUT {
        fn accept(&mut self, c: char) -> AcceptResult {
            match self {
                Self::C => {
                    if c == 'o' {
                        *self = Self::O;
                        return AcceptResult::Accepted;
                    }
                }
                Self::O => match c {
                    'n' => {
                        *self = Self::N;
                        return AcceptResult::Accepted;
                    }
                    'm' => {
                        *self = Self::M;
                        return AcceptResult::Accepted;
                    }
                    _ => {}
                },
                Self::N => match c {
                    'i' => {
                        *self = Self::NI;
                        return AcceptResult::Accepted;
                    }
                    'o' => {
                        *self = Self::NO;
                        return AcceptResult::Accepted;
                    }
                    _ => return AcceptResult::AcceptedAndFinished((3, 1)),
                },
                Self::M => {
                    if let '0'..='9' = c {
                        return AcceptResult::AcceptedAndFinished((3, 0));
                    }
                }
                Self::NI => {
                    if c == 'n' {
                        *self = Self::NIN;
                        return AcceptResult::Accepted;
                    }
                }
                Self::NIN => {
                    if c == '$' {
                        return AcceptResult::AcceptedAndFinished((5, 0));
                    }
                }
                Self::NO => {
                    if c == 'u' {
                        *self = Self::NOU;
                        return AcceptResult::Accepted;
                    }
                }
                Self::NOU => {
                    if c == 't' {
                        *self = Self::NOUT;
                        return AcceptResult::Accepted;
                    }
                }
                Self::NOUT => {
                    if c == '$' {
                        return AcceptResult::AcceptedAndFinished((6, 0));
                    }
                }
            }

            AcceptResult::NoMatch
        }
    }

    #[allow(clippy::upper_case_acronyms)]
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
                Self::T => {
                    if let '0'..='9' = c {
                        return AcceptResult::AcceptedAndFinished((3, 0));
                    }
                }
            }

            AcceptResult::NoMatch
        }
    }

    #[allow(clippy::upper_case_acronyms)]
    enum ReservedName {
        AUX(AUX),
        NUL(NUL),
        PRN(PRN),
        CONOrMOrINOrOUT(CONOrMOrINOrOUT),
        LPT(LPT),
    }

    impl ReservedNameMatch for ReservedName {
        fn accept(&mut self, c: char) -> AcceptResult {
            match self {
                Self::AUX(aux) => aux.accept(c),
                Self::NUL(nul) => nul.accept(c),
                Self::PRN(prn) => prn.accept(c),
                Self::CONOrMOrINOrOUT(conormorinorout) => conormorinorout.accept(c),
                Self::LPT(lpt) => lpt.accept(c),
            }
        }

        fn finish(self) -> Option<(usize, usize)> {
            match self {
                Self::CONOrMOrINOrOUT(CONOrMOrINOrOUT::N) => Some((2, 0)),
                _ => None,
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
            r.replace(ReservedName::CONOrMOrINOrOUT(CONOrMOrINOrOUT::C));
        }
        'l' => {
            r.replace(ReservedName::LPT(LPT::L));
        }
        _ => {
            r.take();
        }
    };

    let split_at_reserved_name_impl = |idx: usize, start_offset: usize, end_offset: usize| {
        debug_assert!(idx >= start_offset);
        let l_end = idx - start_offset;
        let l = unsafe { component.get_unchecked(..l_end) };
        let r_start = idx - end_offset + 1;
        debug_assert!(r_start <= component.len());
        let r = unsafe { component.get_unchecked(r_start..) };
        (l, r)
    };

    let mut reserved_name: Option<ReservedName> = None;

    let mut last_idx = 0;

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
                    AcceptResult::AcceptedAndFinished((start_offset, end_offset)) => {
                        return Some(split_at_reserved_name_impl(idx, start_offset, end_offset));
                    }
                }
            } else {
                restart(c, &mut reserved_name);
            }
        } else {
            reserved_name.take();
        }

        last_idx = idx;
    }

    reserved_name
        .take()
        .and_then(ReservedName::finish)
        .map(|(start_offset, end_offset)| {
            split_at_reserved_name_impl(last_idx, start_offset, end_offset)
        })
}

pub(crate) fn validate_path<P: AsRef<Path>>(path: P) -> Result<(), FilePathError> {
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

                    validate_path_component(comp, || get_path(idx, true))?;

                    // Count the separator.
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

    if path_len == 0 {
        Err(EmptyPath)
    } else if path_len > MAX_PATH_LEN {
        Err(PathTooLong(path_len))
    } else {
        Ok(())
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
        assert_eq!(split_at_reserved_name(nestr!("COM0")).unwrap(), ("", ""));
        assert_eq!(
            split_at_reserved_name(nestr!("fooCOM9")).unwrap(),
            ("foo", "")
        );
        assert_eq!(split_at_reserved_name(nestr!("COM7.")).unwrap(), ("", "."));
        assert_eq!(split_at_reserved_name(nestr!("CON7")).unwrap(), ("", "7"));
        assert_eq!(split_at_reserved_name(nestr!("acon ")).unwrap(), ("a", " "));
        assert_eq!(
            split_at_reserved_name(nestr!(" conin$ .txt")).unwrap(),
            (" ", " .txt")
        );
        assert_eq!(
            split_at_reserved_name(nestr!("CONOUT$.")).unwrap(),
            ("", ".")
        );
        assert_eq!(split_at_reserved_name(nestr!("lpT0")).unwrap(), ("", ""));
        assert_eq!(
            split_at_reserved_name(nestr!("barlpt9")).unwrap(),
            ("bar", "")
        );
    }

    fn validate_path_component_(component: &NonEmptyStr) -> Result<(), FilePathError> {
        validate_path_component(component, PathBuf::new)
    }

    #[allow(non_snake_case)]
    #[test]
    fn InvalidCharacter() {
        assert_eq!(
            validate_path_component_(nestr!("/foo")).err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '/'))
        );
        assert_eq!(
            validate_path_component_(nestr!("f/oo")).err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '/'))
        );
        assert_eq!(
            validate_path_component_(nestr!("foo\\")).err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '\\'))
        );

        assert_eq!(
            validate_path_component_(nestr!("C:foo")).err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), ':'))
        );
        assert_eq!(
            validate_path_component_(nestr!(":foo")).err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), ':'))
        );

        assert_eq!(
            validate_path_component_(nestr!("\"foo\"")).err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '\"'))
        );

        assert_eq!(
            validate_path_component_(nestr!("foo?")).err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '?'))
        );

        assert_eq!(
            validate_path_component_(nestr!("f*oo")).err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '*'))
        );

        assert_eq!(
            validate_path_component_(nestr!("foo<")).err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '<'))
        );
        assert_eq!(
            validate_path_component_(nestr!("foo>")).err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '>'))
        );

        assert_eq!(
            validate_path_component_(nestr!("foo|")).err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '|'))
        );
        assert_eq!(
            validate_path_component_(nestr!("foo\n")).err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '\n'))
        );
        assert_eq!(
            validate_path_component_(nestr!("bar\x1b")).err().unwrap(),
            FilePathError::InvalidCharacter((PathBuf::new(), '\x1b'))
        );

        // But this works.
        assert!(validate_path_component_(nestr!("foo")).is_ok());
        assert!(validate_path_component_(nestr!("βαρ")).is_ok());
    }

    #[allow(non_snake_case)]
    #[test]
    fn ComponentEndsWithAPeriod() {
        assert_eq!(
            validate_path_component_(nestr!("...")).err().unwrap(),
            FilePathError::ComponentEndsWithAPeriod(PathBuf::new())
        );
        assert_eq!(
            validate_path_component_(nestr!("foo.")).err().unwrap(),
            FilePathError::ComponentEndsWithAPeriod(PathBuf::new())
        );
        assert_eq!(
            validate_path_component_(nestr!("NUL.")).err().unwrap(),
            FilePathError::ComponentEndsWithAPeriod(PathBuf::new())
        );
    }

    #[allow(non_snake_case)]
    #[test]
    fn ComponentEndsWithASpace() {
        assert_eq!(
            validate_path_component_(nestr!("foo ")).err().unwrap(),
            FilePathError::ComponentEndsWithASpace(PathBuf::new())
        );
        assert_eq!(
            validate_path_component_(nestr!("foo . ")).err().unwrap(),
            FilePathError::ComponentEndsWithASpace(PathBuf::new())
        );
        assert_eq!(
            validate_path_component_(nestr!("LPT7 ")).err().unwrap(),
            FilePathError::ComponentEndsWithASpace(PathBuf::new())
        );

        // But this works.
        validate_path_component_(nestr!("foo .txt")).unwrap();
    }

    #[allow(non_snake_case)]
    #[test]
    fn ReservedName() {
        assert_eq!(
            validate_path_component_(nestr!("COM0")).err().unwrap(),
            FilePathError::ReservedName(PathBuf::new())
        );
        assert_eq!(
            validate_path_component_(nestr!("COM9")).err().unwrap(),
            FilePathError::ReservedName(PathBuf::new())
        );
        assert_eq!(
            validate_path_component_(nestr!("CON")).err().unwrap(),
            FilePathError::ReservedName(PathBuf::new())
        );
        assert_eq!(
            validate_path_component_(nestr!(" AUX")).err().unwrap(),
            FilePathError::ReservedName(PathBuf::new())
        );
        assert_eq!(
            validate_path_component_(nestr!("NUL.txt")).err().unwrap(),
            FilePathError::ReservedName(PathBuf::new())
        );
        assert_eq!(
            validate_path_component_(nestr!("LPT0 .txt.bmp"))
                .err()
                .unwrap(),
            FilePathError::ReservedName(PathBuf::new())
        );
        assert_eq!(
            validate_path_component_(nestr!("LPT9")).err().unwrap(),
            FilePathError::ReservedName(PathBuf::new())
        );
        assert_eq!(
            validate_path_component_(nestr!("CONIN$.txt"))
                .err()
                .unwrap(),
            FilePathError::ReservedName(PathBuf::new())
        );
        assert_eq!(
            validate_path_component_(nestr!("CONIN$.txt.bmp"))
                .err()
                .unwrap(),
            FilePathError::ReservedName(PathBuf::new())
        );
        assert_eq!(
            validate_path_component_(nestr!("CONOUT$ . bmp"))
                .err()
                .unwrap(),
            FilePathError::ReservedName(PathBuf::new())
        );

        // But this works.
        validate_path_component_(nestr!("faux")).unwrap();
        validate_path_component_(nestr!("COM")).unwrap();
        validate_path_component_(nestr!("COM11")).unwrap();
        validate_path_component_(nestr!("CON1")).unwrap();
        validate_path_component_(nestr!("CONI")).unwrap();
        validate_path_component_(nestr!("CONIN")).unwrap();
        validate_path_component_(nestr!("CONO")).unwrap();
        validate_path_component_(nestr!("CONOU")).unwrap();
        validate_path_component_(nestr!("CONOUT")).unwrap();
        validate_path_component_(nestr!("COM71")).unwrap();
        validate_path_component_(nestr!("LPT")).unwrap();
        validate_path_component_(nestr!("lpt10")).unwrap();
        validate_path_component_(nestr!(".NUL")).unwrap();
        validate_path_component_(nestr!("foo.PRN")).unwrap();
    }
}
