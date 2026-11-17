//! Names that must never become a file name on the PC.
//!
//! On Windows a file called `con`, `nul`, `com1`… **with any extension** (`con.json`,
//! `COM1.svm`) is not a file: it is the device of that name. Writing to it goes to a serial
//! port or nowhere, and reading it can wait for ever. Whatever comes from another device and
//! ends up in a path (an identifier used as a file name, a file asked for by name) is checked
//! with [`is_windows_device_name`] first — on every system, so that a name refused on the PC
//! is refused everywhere.

/// `true` when `name` — a file name, with or without extension — designates a device on
/// Windows: `con`, `prn`, `aux`, `nul`, `com0`…`com9`, `lpt0`…`lpt9` (and their superscript
/// digits), `conin$`, `conout$`, whatever the case, the extension, and the dots or spaces
/// Windows drops at the end of a name.
pub fn is_windows_device_name(name: &str) -> bool {
    // Windows looks at what stands before the first dot, without its trailing spaces.
    let stem = name.split('.').next().unwrap_or("");
    let stem = stem.trim_end_matches(' ').to_ascii_lowercase();
    if matches!(
        stem.as_str(),
        "con" | "prn" | "aux" | "nul" | "conin$" | "conout$"
    ) {
        return true;
    }
    let mut chars = stem.chars();
    let (Some(a), Some(b), Some(c), Some(d), None) = (
        chars.next(),
        chars.next(),
        chars.next(),
        chars.next(),
        chars.next(),
    ) else {
        return false;
    };
    let port = matches!((a, b, c), ('c', 'o', 'm') | ('l', 'p', 't'));
    port && (d.is_ascii_digit() || matches!(d, '\u{b9}' | '\u{b2}' | '\u{b3}'))
}

/// `true` when Windows would not keep `name` as it is: it drops the dots and spaces at the
/// end of a name (`file.` is `file`), so two different names would be the same file.
pub fn has_trailing_dot_or_space(name: &str) -> bool {
    name.ends_with('.') || name.ends_with(' ')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_names_are_recognised_whatever_their_dress() {
        for name in [
            "con",
            "CON",
            "nul",
            "prn",
            "aux",
            "com1",
            "COM9",
            "com0",
            "lpt1",
            "LPT9",
            "con.json",
            "NUL.txt",
            "com1.svm",
            "Aux.json.tmp",
            "con .json",
            "com\u{b9}",
            "conin$",
            "CONOUT$.log",
        ] {
            assert!(is_windows_device_name(name), "{name}");
        }
        for name in [
            "",
            "console",
            "con-2",
            "com",
            "com10",
            "lpt",
            "nul-1",
            "null",
            "icon",
            "x.con",
            "BMW_Monza.svm",
            "my.com1.svm",
            "comx",
        ] {
            assert!(!is_windows_device_name(name), "{name}");
        }
    }

    #[test]
    fn names_windows_would_change() {
        assert!(has_trailing_dot_or_space("file."));
        assert!(has_trailing_dot_or_space("file "));
        assert!(!has_trailing_dot_or_space("file.svm"));
    }
}
