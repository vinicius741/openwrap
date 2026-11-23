//! Keeps OpenWrap's view of a config line identical to OpenVPN's.
//!
//! OpenVPN 2.x reads its config with `fgets` into a 256-byte buffer, so one
//! physical line longer than that is read as several option lines. It refuses
//! an over-long line, but its length check uses `strlen`, which stops at a NUL
//! byte. A line such as `remote x\0<padding> plugin /evil.so` therefore passes
//! OpenWrap's allow-list as a single `remote` directive while OpenVPN runs the
//! hidden `plugin` directive. Rejecting control characters and over-long lines
//! closes that gap; other control characters (vertical tab, form feed, lone
//! carriage return) are separators for OpenVPN but not for OpenWrap's
//! tokenizer, so they are refused too.

use crate::errors::AppError;

/// Longest line OpenVPN 2.x accepts, in bytes, excluding the newline.
pub const MAX_OPENVPN_LINE_BYTES: usize = 254;

/// Returns the first character OpenVPN and OpenWrap could tokenize
/// differently: any control character other than tab.
pub fn find_control_char(line: &str) -> Option<char> {
    line.chars().find(|ch| ch.is_control() && *ch != '\t')
}

pub fn control_char_error(line_no: usize, ch: char) -> AppError {
    AppError::Validation {
        title: "Invalid character in profile".into(),
        message: format!(
            "Line {line_no} contains a control character (U+{:04X}). OpenVPN would read this line differently from OpenWrap.",
            ch as u32
        ),
        directive: None,
        line: Some(line_no),
    }
}

/// Checks a config that OpenWrap is about to store or hand to OpenVPN.
///
/// Every line must be free of control characters (other than tab) and fit in
/// OpenVPN's line buffer, so OpenVPN sees exactly the directives OpenWrap
/// checked.
pub fn validate_openvpn_config_text(text: &str) -> Result<(), AppError> {
    for (index, line) in text.split('\n').enumerate() {
        let line_no = index + 1;
        if let Some(ch) = find_control_char(line) {
            return Err(control_char_error(line_no, ch));
        }
        if line.len() > MAX_OPENVPN_LINE_BYTES {
            let directive = line.split([' ', '\t']).next().unwrap_or_default();
            return Err(AppError::Validation {
                title: "Profile line too long".into(),
                message: format!(
                    "The '{directive}' line is {} bytes long; OpenVPN accepts at most {MAX_OPENVPN_LINE_BYTES}.",
                    line.len()
                ),
                directive: Some(directive.to_string()),
                line: None,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_tabs_and_lines_up_to_the_limit() {
        let longest = format!("remote {}", "a".repeat(MAX_OPENVPN_LINE_BYTES - 7));
        assert_eq!(longest.len(), MAX_OPENVPN_LINE_BYTES);
        validate_openvpn_config_text(&format!("client\n{longest}\nverb\t3\n")).unwrap();
    }

    #[test]
    fn rejects_lines_longer_than_openvpn_reads() {
        let too_long = format!("remote {}", "a".repeat(MAX_OPENVPN_LINE_BYTES - 6));
        assert!(validate_openvpn_config_text(&format!("{too_long}\n")).is_err());
    }

    #[test]
    fn rejects_nul_and_other_control_characters() {
        for ch in ['\0', '\u{0b}', '\u{0c}', '\r', '\u{7f}', '\u{85}'] {
            assert!(
                validate_openvpn_config_text(&format!("remote x{ch}y\n")).is_err(),
                "U+{:04X} was accepted",
                ch as u32
            );
        }
    }
}
