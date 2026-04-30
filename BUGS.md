# quick-xml — Injected Bugs

High-performance XML reader and writer — ETNA workload.

Total mutations: 2

## Bug Index

| # | Variant | Name | Location | Injection | Fix Commit |
|---|---------|------|----------|-----------|------------|
| 1 | `swap_lf_cr_escape_codes_ebaffb3_1` | `swap_lf_cr_escape_codes` | `src/escape.rs:177` | `patch` | `ebaffb3db325c4657e5e5df43afa018ee71ce1c5` |
| 2 | `unescape_predef_entities_0315ed0_1` | `unescape_predef_entities` | `src/events/attributes.rs:86` | `marauders` | `0315ed0a479d3c5b7a1ce8f1bbcde312221e2a2d` |

## Property Mapping

| Variant | Property | Witness(es) |
|---------|----------|-------------|
| `swap_lf_cr_escape_codes_ebaffb3_1` | `EscapeCharCodes` | `witness_escape_char_codes_case_lf`, `witness_escape_char_codes_case_cr` |
| `unescape_predef_entities_0315ed0_1` | `UnescapePredefEntities` | `witness_unescape_predef_entities_case_lt`, `witness_unescape_predef_entities_case_gt`, `witness_unescape_predef_entities_case_combo` |

## Framework Coverage

| Property | proptest | quickcheck | crabcheck | hegel |
|----------|---------:|-----------:|----------:|------:|
| `EscapeCharCodes` | ✓ | ✓ | ✓ | ✓ |
| `UnescapePredefEntities` | ✓ | ✓ | ✓ | ✓ |

## Bug Details

### 1. swap_lf_cr_escape_codes

- **Variant**: `swap_lf_cr_escape_codes_ebaffb3_1`
- **Location**: `src/escape.rs:177` (inside `escape_char`)
- **Property**: `EscapeCharCodes`
- **Witness(es)**:
  - `witness_escape_char_codes_case_lf`
  - `witness_escape_char_codes_case_cr`
- **Source**: [#517](https://github.com/tafia/quick-xml/issues/517) — Fix #517: Fix swapped codes for `\r` and `\n` characters when escaping them
  > The internal `escape_char` writer (used by the xs:list serializer) emitted `&#10;` for `\r` and `&#13;` for `\n`, swapping the two character-reference codes; the fix swapped the match arms back to the spec-required mapping.
- **Fix commit**: `ebaffb3db325c4657e5e5df43afa018ee71ce1c5` — Fix #517: Fix swapped codes for `\r` and `\n` characters when escaping them
- **Invariant violated**: `escape::escape_char` must emit the XML character reference `&#10;` for a `\n` byte (LF, code-point 10) and `&#13;` for a `\r` byte (CR, code-point 13), matching the canonical numeric values.
- **How the mutation triggers**: The buggy body swaps the two match arms (`b'\n' => &#13;` and `b'\r' => &#10;`), so the byte that carries code-point 10 is encoded as `&#13;` and vice versa. Any caller that exercises the whitespace branches of `escape_char` (the xs:list serializer is the in-tree consumer) emits content that disagrees with the XML spec.

### 2. unescape_predef_entities

- **Variant**: `unescape_predef_entities_0315ed0_1`
- **Location**: `src/events/attributes.rs:86` (inside `Attribute::normalized_value`)
- **Property**: `UnescapePredefEntities`
- **Witness(es)**:
  - `witness_unescape_predef_entities_case_lt`
  - `witness_unescape_predef_entities_case_gt`
  - `witness_unescape_predef_entities_case_combo`
- **Source**: [#743](https://github.com/tafia/quick-xml/pull/743) — Fix Attribute::unescape_value does not unescape predefined entities since #739
  > After PR #739 reorganised attribute decoding around `normalized_value_with`, the entry-point method `Attribute::normalized_value` was wired to a `|_| None` resolver instead of `resolve_predefined_entity`, so the five XML predefined entity references (`&lt;`, `&gt;`, `&apos;`, `&quot;`) other than `&amp;` were rejected with `EscapeError::UnrecognizedEntity`.
- **Fix commit**: `0315ed0a479d3c5b7a1ce8f1bbcde312221e2a2d` — Fix Attribute::unescape_value does not unescape predefined entities since #739
- **Invariant violated**: `Attribute::normalized_value` must resolve every XML predefined entity reference (`&lt;`, `&gt;`, `&amp;`, `&apos;`, `&quot;`) to its corresponding character, regardless of whether the caller supplied a custom entity resolver.
- **How the mutation triggers**: The buggy body invokes `self.normalized_value_with(version, 1, |_| None)`. `&amp;` continues to work because `normalize_attr_step` special-cases that name, but `&lt;`, `&gt;`, `&apos;`, and `&quot;` flow through the user resolver, hit the `None` branch, and surface as `EscapeError::UnrecognizedEntity`.
