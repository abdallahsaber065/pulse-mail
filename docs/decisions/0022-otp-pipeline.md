# 0022: Scored OTP and magic-link pipeline, tuned on a labeled corpus

- Status: accepted (2026-10-22)

## Context
The first extractor used one regex plus a few penalties. The user asked for "known methods and fine
tuned ones": more languages, fewer false codes, HTML awareness, magic links,
confidence, and learning from the user. No ML model downloads.

## Known methods we borrowed
- **WebOTP / origin-bound codes**: a last line `@example.com #123456` is
  definitive (confidence 95).
- **iOS / Android / Chrome `one-time-code` autofill heuristics**: a code is
  4-8 characters near a "code" word in many languages, often in the subject
  or first line. Plain words, dates, money and phone numbers are rejected.
- **Password-manager and SMS-retriever style regex sets**: shapes `123456`,
  `123 456`, `123-456`, `G-123456`, `[123456]`, `1234-5678`, and mixed
  uppercase alphanumerics such as `F7K2Q`. Digits are normalized: Arabic-Indic,
  extended Arabic-Indic and full-width.
- **Mail-specific signals**: the sender made the code stand out (bold, `h1`,
  font-size ≥ 18 px, letter-spacing, monospace, or a table cell on its own),
  the code sits alone on a line or after a colon, the sender looks like
  security mail (`noreply`, `security`, `verify`…), and the mail has expiry
  wording.

## Decision
Pipeline (`otp/`):
1. Normalize.
2. Check for a WebOTP line.
3. Gate: a strong keyword in English, Arabic, French, Spanish or German
   (plus a few CJK and Cyrillic forms), or a weak keyword plus HTML emphasis.
4. Score each candidate. Shape, keyword proximity (nearest keyword wins
   against a nearer order/phone/tracking word), linking words ("code is",
   "رمز التحقق الخاص بك هو"), layout, HTML emphasis, sender and expiry add
   points. The following subtract points or drop the candidate:
   - dates, times and phones (spans)
   - money and units
   - URLs and e-mail addresses
   - years
   - footer lines
   - promo words
   - negative words naming the number or the keyword itself ("promo code",
     "postal code", "booking confirmation code", "Meeting ID … Passcode")
   - the Promotions category
5. No code found: look for a one-click sign-in or verification link,
   scoring the URL hints, a long token, the anchor text and the surrounding wording.
   Reset-password, unsubscribe, social and tracking links are excluded.

Confidence is 0-100. **Store ≥ 50** (code chip) and **toast ≥ 60**. Expiry
("expires in 10 minutes" in 5 languages) shows on the toast.

**Learning from the user:** "Not a code" / "Not a link" in the reader
clears the code on that message and gives its sender −30, down to −90. Copying
a code gives +10, up to +20. The adjustment applies when storing and when deciding on a toast.
Data stays local (`otp_feedback`).

**Tuning:** `otp/corpus.rs` has 86 labeled samples in realistic formats. 54
are positive (Google, Microsoft, Apple, GitHub, Amazon, PayPal, Steam,
Facebook, Instagram, X, WhatsApp, Netflix, Uber, Binance, banks, Egyptian
banks and telecoms in Arabic, Fawry, InstaPay, French, Spanish, German, HTML-only
mail, and 5 magic links). 32 are negatives (receipts, shipping, newsletters,
promos, bookings, meeting PINs, statements, footers, password-reset links).
After tuning, the corpus scores precision 1.00 and recall 1.00. A **holdout set of 20** was
written afterwards and never tuned against: precision 1.00, recall 0.93. The
one miss is a split alphanumeric code (`K7Q2-9XMP`), which isn't supported yet.
These are synthetic samples, so real-world numbers will be lower. The
feedback loop covers the rest.

Cached mail is re-scored once per detector version, at startup and in batches
(`otp_version` setting).

## Alternatives
- A small on-device ML model: needs a model download and real labeled data
  we don't have. Weighted rules are explainable and testable.
- Gmail's own OTP features: none in the API.
