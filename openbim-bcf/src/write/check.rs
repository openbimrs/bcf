//! Lexical checks the writer applies before emitting a single byte.
//!
//! Each check mirrors a constraint in the official schemas, so that a value
//! this crate accepts is one every schema-validating reader accepts too.

use super::{Invalid, TargetVersion};

/// `Guid` from `markup.xsd`. 2.1 accepts either case; 3.0's
/// `shared-types.xsd` narrowed the pattern to lowercase hex.
pub(super) fn guid(value: &str, version: TargetVersion) -> Result<(), Invalid> {
    let hex = |c: u8| match version {
        TargetVersion::V2_1 => c.is_ascii_hexdigit(),
        TargetVersion::V3_0 => c.is_ascii_digit() || (b'a'..=b'f').contains(&c),
    };
    let b = value.as_bytes();
    let shaped = b.len() == 36
        && b.iter().enumerate().all(|(i, &c)| match i {
            8 | 13 | 18 | 23 => c == b'-',
            _ => hex(c),
        });
    if shaped {
        Ok(())
    } else {
        Err(Invalid::Guid {
            value: value.to_string(),
        })
    }
}

/// An IFC `GlobalId`: 22 characters of the IFC base64 alphabet
/// `0-9A-Za-z_$`.
///
/// The first character must also be `0`–`3`: 22 characters carry 132 bits and
/// a GUID has 128, so the leading character encodes only two. The schema's
/// regex does not say so, but a value outside that range decodes to no GUID.
pub(super) fn ifc_guid(value: &str) -> Result<(), Invalid> {
    let b = value.as_bytes();
    let alphabet = |c: &u8| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'$';
    if b.len() == 22 && b.iter().all(alphabet) && (b'0'..=b'3').contains(&b[0]) {
        Ok(())
    } else {
        Err(Invalid::IfcGuid {
            value: value.to_string(),
        })
    }
}

/// A `Color` attribute: 6 or 8 hex digits. 2.1's `visinfo.xsd` pattern is
/// `[0-9,A-F]{6}([0-9,A-F]{2})?` — uppercase only (the comma is a typo in
/// the regex, not a digit); 3.0 widened it to `[0-9A-Fa-f]`.
pub(super) fn color(value: &str, version: TargetVersion) -> Result<(), Invalid> {
    let digit = |c: u8| match version {
        TargetVersion::V2_1 => c.is_ascii_digit() || (b'A'..=b'F').contains(&c),
        TargetVersion::V3_0 => c.is_ascii_hexdigit(),
    };
    if matches!(value.len(), 6 | 8) && value.bytes().all(digit) {
        Ok(())
    } else {
        Err(Invalid::Color {
            value: value.to_string(),
        })
    }
}

/// A text value the reader will return verbatim.
///
/// Refused: empty or blank (3.0 types these `NonEmptyOrBlankString`, and the
/// reader reads blank as absent), surrounding whitespace (the reader trims,
/// so it would not survive a round trip), and characters XML 1.0 cannot
/// carry at all.
pub(super) fn text(value: &str) -> Result<(), Invalid> {
    if value.trim().is_empty() {
        return Err(Invalid::Blank);
    }
    if value.trim() != value {
        return Err(Invalid::SurroundingWhitespace {
            value: value.to_string(),
        });
    }
    if let Some(ch) = value.chars().find(|&c| !xml_char(c)) {
        return Err(Invalid::ForbiddenCharacter { ch });
    }
    Ok(())
}

/// XML 1.0 `Char`: `#x9 | #xA | #xD | [#x20-#xD7FF] | [#xE000-#xFFFD] |
/// [#x10000-#x10FFFF]`. Rust `char` already excludes surrogates.
fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r') || (c >= ' ' && c != '\u{FFFE}' && c != '\u{FFFF}')
}

/// The lexical space of XSD 1.0 `xs:dateTime`:
/// `-?YYYY-MM-DDThh:mm:ss(.s+)?(Z|(+|-)hh:mm)?`.
///
/// Calendar validity is checked too — `2023-02-30` matches the regex shape
/// but not the type — because a schema validator rejects it.
pub(super) fn date_time(value: &str) -> Result<(), Invalid> {
    if parse_date_time(value).is_some() {
        Ok(())
    } else {
        Err(Invalid::DateTime {
            value: value.to_string(),
        })
    }
}

fn parse_date_time(value: &str) -> Option<()> {
    let s = value.strip_prefix('-').unwrap_or(value);
    let (date, rest) = s.split_once('T')?;

    // Date: a year of four or more digits (no leading zero beyond four, and
    // never 0000 in XSD 1.0), then -MM-DD.
    let mut parts = date.rsplitn(3, '-');
    let (day, month, year) = (parts.next()?, parts.next()?, parts.next()?);
    if year.len() < 4 || (year.len() > 4 && year.starts_with('0')) || !digits(year) {
        return None;
    }
    if year.bytes().all(|b| b == b'0') {
        return None;
    }
    let month = two_digits(month)?;
    let day = two_digits(day)?;
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }

    // Time zone: Z, ±hh:mm with |offset| <= 14:00, or none.
    let (time, zone) = if let Some(t) = rest.strip_suffix('Z') {
        (t, None)
    } else if rest.len() > 6 && matches!(rest.as_bytes()[rest.len() - 6], b'+' | b'-') {
        let (t, z) = rest.split_at(rest.len() - 6);
        (t, Some(&z[1..]))
    } else {
        (rest, None)
    };
    if let Some(z) = zone {
        let (h, m) = z.split_once(':')?;
        let (h, m) = (two_digits(h)?, two_digits(m)?);
        if m > 59 || h > 14 || (h == 14 && m != 0) {
            return None;
        }
    }

    // Time: hh:mm:ss with an optional fraction. 24:00:00 is the one legal
    // end-of-day spelling.
    let (clock, fraction) = match time.split_once('.') {
        Some((c, f)) if !f.is_empty() && digits(f) => (c, Some(f)),
        Some(_) => return None,
        None => (time, None),
    };
    let mut hms = clock.split(':');
    let (h, m, sec) = (
        two_digits(hms.next()?)?,
        two_digits(hms.next()?)?,
        two_digits(hms.next()?)?,
    );
    if hms.next().is_some() {
        return None;
    }
    let end_of_day =
        h == 24 && m == 0 && sec == 0 && fraction.is_none_or(|f| f.bytes().all(|b| b == b'0'));
    if !end_of_day && (h > 23 || m > 59 || sec > 59) {
        return None;
    }
    Some(())
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn two_digits(s: &str) -> Option<u32> {
    (s.len() == 2 && digits(s))
        .then(|| s.parse().ok())
        .flatten()
}

fn days_in_month(year: &str, month: u32) -> u32 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 => {
            // Leap-year rule on the last four digits is exact: 400 divides
            // 10 000, so higher digits never change the answer.
            let y: u32 = year[year.len() - 4..].parse().unwrap_or(1);
            if (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400) {
                29
            } else {
                28
            }
        }
        _ => 31,
    }
}

/// A finite `xs:double`. NaN and infinities are legal XSD lexemes (`NaN`,
/// `INF`) but meaningless in a camera, and Rust would spell them differently.
pub(super) fn finite(value: f64) -> Result<(), Invalid> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Invalid::Number {
            value,
            expected: "a finite number",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guids_follow_the_version_pattern() {
        let lower = "3f2504e0-4f89-41d3-9a0c-0305e82c3301";
        let upper = "3F2504E0-4F89-41D3-9A0C-0305E82C3301";
        assert!(guid(lower, TargetVersion::V2_1).is_ok());
        assert!(guid(lower, TargetVersion::V3_0).is_ok());
        assert!(guid(upper, TargetVersion::V2_1).is_ok());
        assert!(
            guid(upper, TargetVersion::V3_0).is_err(),
            "3.0 is lowercase only"
        );
        for bad in [
            "",
            "3f2504e0-4f89-41d3-9a0c-0305e82c330",
            "3f2504e04f8941d39a0c0305e82c3301",
            "{3f2504e0-4f89-41d3-9a0c-0305e82c3301}",
            "3f2504e0-4f89-41d3-9a0c-0305e82c330g",
            "3f2504e0_4f89-41d3-9a0c-0305e82c3301",
        ] {
            assert!(guid(bad, TargetVersion::V2_1).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn ifc_guids_are_22_chars_of_ifc_base64() {
        assert!(ifc_guid("0fXw$sQh19ixbI4tZgfkXu").is_ok());
        assert!(ifc_guid("3cUkl32yn9qRSPvBJVyWY_").is_ok());
        for bad in [
            "",
            "0fXw$sQh19ixbI4tZgfkX",   // 21
            "0fXw$sQh19ixbI4tZgfkXuu", // 23
            "0fXw$sQh19ixbI4tZgfk,u",  // comma: a typo in the 2.1 regex
            "0fXw-sQh19ixbI4tZgfkXu",  // not in the alphabet
            "4fXw$sQh19ixbI4tZgfkXu",  // leading char encodes > 2 bits
        ] {
            assert!(ifc_guid(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn colors_are_6_or_8_hex_digits_in_the_version_case() {
        for ok in ["FF0000", "80FF0000", "00AAFF", "0A0B0C0D"] {
            assert!(color(ok, TargetVersion::V2_1).is_ok(), "{ok}");
            assert!(color(ok, TargetVersion::V3_0).is_ok(), "{ok}");
        }
        assert!(color("ff0000", TargetVersion::V3_0).is_ok());
        assert!(
            color("ff0000", TargetVersion::V2_1).is_err(),
            "2.1 is uppercase only"
        );
        for bad in [
            "",
            "FFF",
            "FF000",
            "FF00000",
            "FF0000000",
            "#FF0000",
            "GG0000",
            "FF,000",
        ] {
            assert!(color(bad, TargetVersion::V2_1).is_err(), "{bad}");
            assert!(color(bad, TargetVersion::V3_0).is_err(), "{bad}");
        }
    }

    #[test]
    fn text_refuses_what_would_not_round_trip() {
        assert!(text("Kollision Lüftung & <Rohr>").is_ok());
        assert!(text("line one\r\nline two").is_ok());
        assert_eq!(text(""), Err(Invalid::Blank));
        assert_eq!(text(" \t\n"), Err(Invalid::Blank));
        assert!(matches!(
            text(" x"),
            Err(Invalid::SurroundingWhitespace { .. })
        ));
        assert!(matches!(
            text("x\n"),
            Err(Invalid::SurroundingWhitespace { .. })
        ));
        assert_eq!(
            text("a\u{0}b"),
            Err(Invalid::ForbiddenCharacter { ch: '\u{0}' })
        );
        assert_eq!(
            text("a\u{1b}b"),
            Err(Invalid::ForbiddenCharacter { ch: '\u{1b}' })
        );
        assert_eq!(
            text("a\u{FFFF}"),
            Err(Invalid::ForbiddenCharacter { ch: '\u{FFFF}' })
        );
    }

    #[test]
    fn date_times_accept_the_xsd_lexical_space() {
        for ok in [
            "2026-09-26T10:00:00Z",
            "2026-09-26T10:00:00",
            "2026-09-26T10:00:00.123456+02:00",
            "2026-09-26T10:00:00-14:00",
            "2024-02-29T00:00:00Z",
            "2000-02-29T00:00:00Z",
            "2026-12-31T24:00:00Z",
            "-0044-03-15T12:00:00Z",
            "12026-01-01T00:00:00Z",
        ] {
            assert!(date_time(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "2026-09-26",
            "2026-09-26 10:00:00",
            "2026-9-26T10:00:00Z",
            "26-09-26T10:00:00Z",
            "0000-01-01T00:00:00Z",
            "02026-01-01T00:00:00Z",
            "2026-13-01T00:00:00Z",
            "2026-02-29T00:00:00Z",
            "1900-02-29T00:00:00Z",
            "2026-04-31T00:00:00Z",
            "2026-09-26T24:00:01Z",
            "2026-09-26T10:60:00Z",
            "2026-09-26T10:00:60Z",
            "2026-09-26T10:00Z",
            "2026-09-26T10:00:00.Z",
            "2026-09-26T10:00:00+15:00",
            "2026-09-26T10:00:00+14:30",
            "2026-09-26T10:00:00+0200",
            "2026-09-26T10:00:00z",
        ] {
            assert!(date_time(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn non_finite_numbers_are_refused() {
        assert!(finite(0.0).is_ok());
        assert!(finite(f64::NAN).is_err());
        assert!(finite(f64::INFINITY).is_err());
    }
}
