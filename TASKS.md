# quick-xml — ETNA Tasks

Total tasks: 8

## Task Index

| Task | Variant | Framework | Property | Witness |
|------|---------|-----------|----------|---------|
| 001 | `swap_lf_cr_escape_codes_ebaffb3_1` | proptest | `EscapeCharCodes` | `witness_escape_char_codes_case_lf` |
| 002 | `swap_lf_cr_escape_codes_ebaffb3_1` | quickcheck | `EscapeCharCodes` | `witness_escape_char_codes_case_lf` |
| 003 | `swap_lf_cr_escape_codes_ebaffb3_1` | crabcheck | `EscapeCharCodes` | `witness_escape_char_codes_case_lf` |
| 004 | `swap_lf_cr_escape_codes_ebaffb3_1` | hegel | `EscapeCharCodes` | `witness_escape_char_codes_case_lf` |
| 005 | `unescape_predef_entities_0315ed0_1` | proptest | `UnescapePredefEntities` | `witness_unescape_predef_entities_case_lt` |
| 006 | `unescape_predef_entities_0315ed0_1` | quickcheck | `UnescapePredefEntities` | `witness_unescape_predef_entities_case_lt` |
| 007 | `unescape_predef_entities_0315ed0_1` | crabcheck | `UnescapePredefEntities` | `witness_unescape_predef_entities_case_lt` |
| 008 | `unescape_predef_entities_0315ed0_1` | hegel | `UnescapePredefEntities` | `witness_unescape_predef_entities_case_lt` |

## Witness Catalog

- `witness_escape_char_codes_case_lf` — base passes, variant fails
- `witness_escape_char_codes_case_cr` — base passes, variant fails
- `witness_unescape_predef_entities_case_lt` — base passes, variant fails
- `witness_unescape_predef_entities_case_gt` — base passes, variant fails
- `witness_unescape_predef_entities_case_combo` — base passes, variant fails
