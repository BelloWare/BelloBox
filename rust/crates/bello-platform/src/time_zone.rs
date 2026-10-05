//! Read-only system time-zone discovery. No offset-to-region guesses.

#[cfg(any(target_os = "linux", target_os = "macos", test))]
const MAX_IDENTIFIER_BYTES: usize = 255;

/// Returns the system's time-zone identifier when it can be read safely.
///
/// This validates syntax, not membership in a time-zone database. Callers must
/// validate the returned identifier (including legacy aliases) against their
/// own catalog and explicitly disclose any UTC fallback. No subprocesses,
/// network requests, permission prompts, or preference writes are performed.
///
/// Linux honors an identifier-only `TZ` override, then reads the known
/// `/etc/localtime` symlink or a bounded `/etc/timezone`. Arbitrary `TZ` file
/// paths and POSIX rules are not interpreted. A present but unusable override
/// returns `None` rather than silently substituting the machine's zone.
/// macOS uses CoreFoundation's system zone; that API may itself return GMT
/// when discovery fails, which it does not distinguish from configured GMT.
/// Unsupported platforms return `None`.
pub fn system_time_zone_identifier() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        linux_identifier()
    }
    #[cfg(target_os = "macos")]
    {
        macos::identifier()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

// Only literal database keys are returned. In particular, numeric offsets,
// traversal, paths, control characters, and POSIX rule punctuation are rejected.
// Database membership remains the caller's responsibility; aliases such as
// US/Eastern, CET, and CST6CDT must not be guessed or rewritten here.
#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn parse_identifier(value: &str) -> Option<String> {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        return None;
    }
    if !value.split('/').all(|part| {
        part.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'+'))
    }) {
        return None;
    }
    Some(value.to_owned())
}

#[cfg(any(target_os = "linux", test))]
fn parse_tz_override(value: &str) -> Option<String> {
    if value.len() > MAX_IDENTIFIER_BYTES + 1 {
        return None;
    }
    parse_identifier(value.strip_prefix(':').unwrap_or(value))
}

#[cfg(any(target_os = "linux", test))]
fn parse_localtime_link(value: &str) -> Option<String> {
    if value.len() > 512 {
        return None;
    }
    // A relative symlink is relative to /etc. Inspect text only: never follow
    // an environment-supplied path or read the linked binary zoneinfo file.
    [
        "/usr/share/zoneinfo/",
        "/usr/share/lib/zoneinfo/",
        "/etc/zoneinfo/",
        "../usr/share/zoneinfo/",
        "../usr/share/lib/zoneinfo/",
        "zoneinfo/",
    ]
    .iter()
    .find_map(|prefix| value.strip_prefix(prefix).and_then(parse_identifier))
}

#[cfg(any(target_os = "linux", test))]
fn parse_timezone_file(value: &[u8]) -> Option<String> {
    if value.len() > MAX_IDENTIFIER_BYTES + 2 {
        return None;
    }
    let value = std::str::from_utf8(value).ok()?;
    // Permit one customary line ending, not embedded lines or arbitrary trim.
    let value = value.strip_suffix('\n').unwrap_or(value);
    let value = value.strip_suffix('\r').unwrap_or(value);
    parse_identifier(value)
}

#[cfg(target_os = "linux")]
fn linux_identifier() -> Option<String> {
    if let Some(value) = std::env::var_os("TZ") {
        return value.to_str().and_then(parse_tz_override);
    }
    if let Some(identifier) = std::fs::read_link("/etc/localtime")
        .ok()
        .and_then(|path| path.to_str().and_then(parse_localtime_link))
    {
        return Some(identifier);
    }
    read_timezone_file(std::path::Path::new("/etc/timezone"))
}

#[cfg(target_os = "linux")]
fn read_timezone_file(path: &std::path::Path) -> Option<String> {
    use std::io::Read;

    // Reject configured devices/FIFOs before open (which can otherwise block).
    // This is a trusted system-config path, not an attacker-controlled input.
    // Recheck the opened handle, but the metadata/open race and filesystem I/O
    // are not wall-clock bounded; replacing system files requires OS authority.
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take((MAX_IDENTIFIER_BYTES + 3) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    parse_timezone_file(&bytes)
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{parse_identifier, MAX_IDENTIFIER_BYTES};
    use std::ffi::{c_char, c_void};
    use std::ptr::NonNull;

    // SDK C signatures: CFIndex is signed pointer-width; Boolean is UInt8.
    // https://developer.apple.com/documentation/corefoundation/cftimezonecopysystem()
    // https://developer.apple.com/documentation/corefoundation/cftimezonegetname(_:)
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFTimeZoneCopySystem() -> *const c_void;
        fn CFTimeZoneGetName(zone: *const c_void) -> *const c_void;
        fn CFStringGetLength(value: *const c_void) -> isize;
        fn CFStringGetCString(
            value: *const c_void,
            buffer: *mut c_char,
            buffer_size: isize,
            encoding: u32,
        ) -> u8;
        fn CFRelease(value: *const c_void);
    }

    struct OwnedTimeZone(NonNull<c_void>);

    impl Drop for OwnedTimeZone {
        fn drop(&mut self) {
            // SAFETY: CopySystem follows the Create Rule. This guard owns its
            // sole retained reference and releases it exactly once.
            unsafe { CFRelease(self.0.as_ptr()) };
        }
    }

    pub(super) fn identifier() -> Option<String> {
        // SAFETY: CopySystem takes no arguments and returns an owned reference
        // or null. GetName borrows a CFString kept alive by the timezone guard.
        unsafe {
            let zone = OwnedTimeZone(NonNull::new(CFTimeZoneCopySystem().cast_mut())?);
            let name = CFTimeZoneGetName(zone.0.as_ptr());
            if name.is_null() {
                return None;
            }
            let length = CFStringGetLength(name);
            if length <= 0 || length > MAX_IDENTIFIER_BYTES as isize {
                return None;
            }
            let mut bytes = [0_u8; MAX_IDENTIFIER_BYTES + 1];
            // UTF-8; failure or insufficient space never yields a partial ID.
            if CFStringGetCString(
                name,
                bytes.as_mut_ptr().cast(),
                bytes.len() as isize,
                0x0800_0100,
            ) == 0
            {
                return None;
            }
            let end = bytes.iter().position(|b| *b == 0)?;
            // Valid identifiers are ASCII. Check UTF-16 length as well so an
            // embedded NUL cannot turn a longer CFString into a valid prefix.
            if end != length as usize {
                return None;
            }
            parse_identifier(std::str::from_utf8(&bytes[..end]).ok()?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_and_aliases_are_preserved() {
        for id in [
            "UTC",
            "Etc/UTC",
            "GMT",
            "US/Eastern",
            "CST6CDT",
            "Asia/Kolkata",
            "Asia/Calcutta",
            "America/Argentina/Buenos_Aires",
            "Etc/GMT+5",
        ] {
            assert_eq!(parse_identifier(id).as_deref(), Some(id));
        }
    }

    #[test]
    fn invalid_identifiers_are_not_guessed() {
        for id in [
            "",
            "/etc/localtime",
            "../Asia/Tokyo",
            "Asia/../Tokyo",
            "Asia//Tokyo",
            "Asia/Tokyo/",
            " Asia/Tokyo",
            "Asia/Tokyo\n",
            "Asia/Tokyo\0",
            "日本/Tokyo",
            "+05:30",
            "-0800",
            "UTC+05:00",
            "EST5EDT,M3.2.0,M11.1.0",
        ] {
            assert!(parse_identifier(id).is_none(), "{id:?}");
        }
        assert!(parse_identifier(&"A".repeat(MAX_IDENTIFIER_BYTES)).is_some());
        assert!(parse_identifier(&"A".repeat(MAX_IDENTIFIER_BYTES + 1)).is_none());
    }

    #[test]
    fn overrides_accept_identifiers_but_never_paths() {
        assert_eq!(
            parse_tz_override(":US/Eastern").as_deref(),
            Some("US/Eastern")
        );
        assert_eq!(parse_tz_override("UTC").as_deref(), Some("UTC"));
        for value in [
            "",
            ":",
            "::UTC",
            ":/usr/share/zoneinfo/UTC",
            "/tmp/zone",
            "UTC\n",
        ] {
            assert!(parse_tz_override(value).is_none());
        }
        assert!(parse_tz_override(&"A".repeat(257)).is_none());
    }

    #[test]
    fn known_system_link_fixtures_are_literal() {
        for value in [
            "/usr/share/zoneinfo/Europe/Paris",
            "../usr/share/zoneinfo/Europe/Paris",
            "/usr/share/lib/zoneinfo/Europe/Paris",
            "/etc/zoneinfo/Europe/Paris",
            "zoneinfo/Europe/Paris",
        ] {
            assert_eq!(parse_localtime_link(value).as_deref(), Some("Europe/Paris"));
        }
        for value in [
            "/tmp/zoneinfo/Europe/Paris",
            "/usr/share/zoneinfo/../private",
            "../../usr/share/zoneinfo/Europe/Paris",
            "/usr/share/zoneinfo/",
            "Europe/Paris",
        ] {
            assert!(parse_localtime_link(value).is_none());
        }
        assert!(parse_localtime_link(&"A".repeat(513)).is_none());
    }

    #[test]
    fn timezone_file_is_single_line_utf8_and_bounded() {
        for value in [b"Asia/Tokyo".as_slice(), b"Asia/Tokyo\n", b"Asia/Tokyo\r\n"] {
            assert_eq!(parse_timezone_file(value).as_deref(), Some("Asia/Tokyo"));
        }
        for value in [
            b"Asia/Tokyo\nUTC".as_slice(),
            b"UTC\n\n",
            b"\xff",
            b" UTC ",
            b"UTC\0",
        ] {
            assert!(parse_timezone_file(value).is_none());
        }
        assert!(parse_timezone_file(&vec![b'A'; 258]).is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn timezone_file_reader_rejects_nonregular_files_before_open() {
        use std::ffi::{c_char, CString};
        use std::os::unix::ffi::OsStrExt;
        use std::time::{SystemTime, UNIX_EPOCH};

        // Linux libc ABI: mode_t is unsigned int. Only creates this test's FIFO;
        // the resolver itself uses no C filesystem API or subprocess.
        extern "C" {
            fn mkfifo(path: *const c_char, mode: u32) -> i32;
        }
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = Fixture(
            std::env::temp_dir().join(format!("bello-time-zone-{}-{nonce}", std::process::id())),
        );
        std::fs::create_dir(&fixture.0).unwrap();
        assert_eq!(read_timezone_file(&fixture.0), None);
        assert_eq!(read_timezone_file(&fixture.0.join("missing")), None);
        let regular = fixture.0.join("timezone");
        std::fs::write(&regular, b"Europe/Paris\n").unwrap();
        assert_eq!(
            read_timezone_file(&regular).as_deref(),
            Some("Europe/Paris")
        );
        std::fs::write(&regular, vec![b'A'; MAX_IDENTIFIER_BYTES + 3]).unwrap();
        assert_eq!(read_timezone_file(&regular), None);
        let fifo = fixture.0.join("fifo");
        let c_path = CString::new(fifo.as_os_str().as_bytes()).unwrap();
        // SAFETY: c_path is NUL-terminated and remains live for the call.
        assert_eq!(unsafe { mkfifo(c_path.as_ptr(), 0o600) }, 0);
        assert_eq!(read_timezone_file(&fifo), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_system_resolver_returns_a_bounded_identifier() {
        // Read-only integration smoke test; does not set/reset system defaults.
        let id = system_time_zone_identifier().expect("macOS supplies a system zone or GMT");
        assert_eq!(parse_identifier(&id).as_deref(), Some(id.as_str()));
    }
}
