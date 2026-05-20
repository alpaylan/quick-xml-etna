//! Witness tests for the quick-xml ETNA workload.
//!
//! Each `witness_*` test calls one of the `property_*` functions in
//! `quick_xml::etna` with frozen inputs. Tests pass on the base commit and
//! fail when the corresponding mutation is active.

use quick_xml::etna::{
    property_escape_char_codes, property_unescape_predef_entities, PropertyResult,
};

fn assert_pass(r: PropertyResult) {
    match r {
        PropertyResult::Pass => {}
        PropertyResult::Fail(m) => panic!("property failed: {m}"),
        PropertyResult::Discard => panic!("property unexpectedly discarded"),
    }
}

// ---- unescape_predef_entities_0315ed0_1 ----
//
// Attribute::normalized_value must resolve the five XML predefined entities.
// The mutation passes `|_| None` to normalized_value_with, so any input that
// contains one of those references produces an UnrecognizedEntity error.

#[test]
fn witness_unescape_predef_entities_case_lt() {
    // "&lt;" must decode to "<".
    assert_pass(property_unescape_predef_entities(b"&lt;".to_vec()));
}

#[test]
fn witness_unescape_predef_entities_case_gt() {
    // "&gt;" must decode to ">".
    assert_pass(property_unescape_predef_entities(b"&gt;".to_vec()));
}

#[test]
fn witness_unescape_predef_entities_case_combo() {
    // Multiple entities mixed with literal letters round-trip correctly.
    // Note: `&amp;` is special-cased inside normalize_attr_step (it always
    // resolves to `&` regardless of the user-supplied entity resolver), so
    // we include `&lt;` / `&gt;` / `&quot;` / `&apos;` to make sure the
    // missing-resolver bug is actually triggered.
    assert_pass(property_unescape_predef_entities(
        b"a&lt;b&amp;c&gt;d&quot;e&apos;f".to_vec(),
    ));
}

// ---- swap_lf_cr_escape_codes_ebaffb3_1 ----
//
// escape_char must emit "&#10;" for '\n' and "&#13;" for '\r'. The mutation
// swaps the two arms.

#[test]
fn witness_escape_char_codes_case_lf() {
    assert_pass(property_escape_char_codes(b'\n'));
}

#[test]
fn witness_escape_char_codes_case_cr() {
    assert_pass(property_escape_char_codes(b'\r'));
}

#[test]
fn witness_escape_char_codes_case_lt() {
    // Sanity: the unrelated arms still pass — confirms the property and not
    // the binding itself is broken when the mutation is active.
    assert_pass(property_escape_char_codes(b'<'));
}
