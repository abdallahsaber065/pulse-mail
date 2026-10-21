# 0008: All text slicing is char-boundary safe

- Status: accepted (2026-10-21)

## Context
`in_url_token` computed `rfind(char::is_whitespace) + 1`. A non-breaking
space (U+00A0) is 2 bytes in UTF-8, so the slice started inside a character
and panicked ("byte index 339 is not a char boundary"). Real mail is full of
NBSP, emoji, zero-width joiners and Arabic.

## Decision
- Never slice a `&str` with an index derived from byte arithmetic on a
  character. Use `char_indices()` and add `c.len_utf8()`, or iterate `chars()`.
- Truncation checks `is_char_boundary` (see `mime.rs`) or counts chars (`notify::ellipsize`).
- Regression tests with NBSP, em space, ideographic space, zero-width
  characters, BOM, emoji, Arabic letters and Arabic-Indic digits inserted at
  every position (`otp::tests::never_panics_on_multibyte_text`).

## Consequences
Reviewers grep for `[..`/`[i..` style slicing in new text code and ask for
a test with multibyte input.
