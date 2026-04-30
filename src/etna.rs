//! ETNA workload harness for quick-xml.
//!
//! Each `property_*` function below is the framework-neutral kernel for one
//! mined bug. Witness tests, the `etna` binary, and every framework adapter
//! call into the same function so the same invariant is exercised everywhere.

#![allow(missing_docs)]

use std::borrow::Cow;
use std::fmt::Write;

use crate::escape::escape_char;
use crate::events::attributes::Attribute;
use crate::name::QName;
use crate::XmlVersion;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyResult {
    Pass,
    Fail(String),
    Discard,
}

/// `Attribute::normalized_value` must resolve the five XML predefined entities
/// (`&lt;`, `&gt;`, `&amp;`, `&apos;`, `&quot;`).
///
/// Bug `unescape_predef_entities_0315ed0_1` violates this by passing
/// `|_| None` to `normalized_value_with`, so `&lt;` is left unresolved (the
/// call returns `EscapeError::UnrecognizedEntity`).
pub fn property_unescape_predef_entities(input: Vec<u8>) -> PropertyResult {
    if !is_predefined_entity_input(&input) {
        return PropertyResult::Discard;
    }
    let attr = Attribute {
        key: QName(b"x"),
        value: Cow::Owned(input.clone()),
    };
    let got = match attr.normalized_value(XmlVersion::V1_0) {
        Ok(v) => v.into_owned(),
        Err(e) => {
            return PropertyResult::Fail(format!(
                "normalized_value failed on {:?}: {:?}",
                String::from_utf8_lossy(&input),
                e
            ))
        }
    };
    let expected = expected_predef_unescape(&input);
    if got == expected {
        PropertyResult::Pass
    } else {
        PropertyResult::Fail(format!(
            "input={:?} expected={:?} got={:?}",
            String::from_utf8_lossy(&input),
            expected,
            got
        ))
    }
}

/// Restrict the property domain to byte sequences that are valid UTF-8 and
/// only contain the five XML predefined entity references plus literal ASCII
/// letters / digits. We discard anything else so the property has a single
/// well-defined oracle (`expected_predef_unescape`).
fn is_predefined_entity_input(input: &[u8]) -> bool {
    if std::str::from_utf8(input).is_err() {
        return false;
    }
    let mut i = 0;
    while i < input.len() {
        let b = input[i];
        if b == b'&' {
            let rest = &input[i + 1..];
            for ent in &[
                &b"lt;"[..],
                &b"gt;"[..],
                &b"amp;"[..],
                &b"apos;"[..],
                &b"quot;"[..],
            ] {
                if rest.starts_with(ent) {
                    i += 1 + ent.len();
                    continue;
                }
            }
            // Bare `&` not starting a known entity is invalid for this property.
            if !(rest.starts_with(b"lt;")
                || rest.starts_with(b"gt;")
                || rest.starts_with(b"amp;")
                || rest.starts_with(b"apos;")
                || rest.starts_with(b"quot;"))
            {
                return false;
            }
        } else if b.is_ascii_alphanumeric() {
            i += 1;
        } else {
            // Anything else (including whitespace) is rejected so the oracle
            // stays simple — normalization changes whitespace-handling, and
            // the bug we mine is purely about entity expansion.
            return false;
        }
    }
    true
}

fn expected_predef_unescape(input: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < input.len() {
        let b = input[i];
        if b == b'&' {
            let rest = &input[i + 1..];
            if rest.starts_with(b"lt;") {
                out.push('<');
                i += 4;
            } else if rest.starts_with(b"gt;") {
                out.push('>');
                i += 4;
            } else if rest.starts_with(b"amp;") {
                out.push('&');
                i += 5;
            } else if rest.starts_with(b"apos;") {
                out.push('\'');
                i += 6;
            } else if rest.starts_with(b"quot;") {
                out.push('"');
                i += 6;
            } else {
                out.push(b as char);
                i += 1;
            }
        } else {
            out.push(b as char);
            i += 1;
        }
    }
    out
}

/// `escape::escape_char` (the writer used by the xs:list serializer) must
/// emit `&#10;` for a `\n` byte and `&#13;` for a `\r` byte.
///
/// Bug `swap_lf_cr_escape_codes_ebaffb3_1` swaps the two rules so `\n` ends
/// up as `&#13;` and `\r` as `&#10;`.
pub fn property_escape_char_codes(input: u8) -> PropertyResult {
    let allowed = matches!(
        input,
        b'<' | b'>' | b'\'' | b'&' | b'"' | b'\t' | b'\n' | b'\r' | b' '
    );
    if !allowed {
        return PropertyResult::Discard;
    }
    let s = std::str::from_utf8(&[input]).unwrap().to_string();
    let mut out = String::new();
    if let Err(e) = escape_char(&mut out, &s, 0, 0) {
        return PropertyResult::Fail(format!("escape_char fmt error: {e}"));
    }
    let expected: &str = match input {
        b'<' => "&lt;",
        b'>' => "&gt;",
        b'\'' => "&apos;",
        b'&' => "&amp;",
        b'"' => "&quot;",
        b'\t' => "&#9;",
        b'\n' => "&#10;",
        b'\r' => "&#13;",
        b' ' => "&#32;",
        _ => unreachable!(),
    };
    if out == expected {
        PropertyResult::Pass
    } else {
        PropertyResult::Fail(format!(
            "escape_char({:?}) expected {:?} got {:?}",
            input as char, expected, out
        ))
    }
}

/// Convenience wrapper used by the witness tests so they don't need to
/// allocate an explicit buffer.
pub fn escape_char_to_string(b: u8) -> String {
    let s = std::str::from_utf8(&[b]).unwrap().to_string();
    let mut out = String::new();
    escape_char(&mut out, &s, 0, 0).unwrap();
    out
}

// Silence unused-import warnings in builds that compile lib without tests.
const _: fn() -> Result<(), std::fmt::Error> = || {
    let mut s = String::new();
    s.write_str("ok")
};
