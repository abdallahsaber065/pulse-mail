//! Word lists and patterns for the OTP scorer. Multilingual on purpose:
//! English, Arabic, French, Spanish, German, plus a few common CJK/Cyrillic
//! forms. Arabic words have no `\b` guards because of attached prefixes (ال، و).

use regex::Regex;
use std::sync::LazyLock;

/// Words naming the code itself: a nearby number is very likely the code.
pub static STRONG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b(?:codes?|otp|passcodes?|pass code|pin|one[- ]?time (?:password|pin|passcode|code)|2fa|mfa|tan|",
        r"c[oó]digo|clave|mot de passe|kode|kod|код|einmalpasswort)\b",
        r"|(?i)(?:best[aä]tigungscode|sicherheitscode|verifizierungscode|anmeldecode|zugangscode|einmalcode)",
        r"|رمز|كود|كلمة (?:المرور|السر)|لمرة واحدة|رقم سري|الرقم السري",
        r"|验证码|認証コード|確認コード|인증번호|인증 코드"
    ))
    .unwrap()
});

/// Context words (sign-in, verification) that support a code but don't name it.
pub static WEAK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b(?:verify|verification|verifying|sign[- ]?in|log[- ]?in|login|security|authenticat\w*|confirm\w*|",
        r"password|identity|access|v[eé]rification|connexion|verificaci[oó]n|iniciar sesi[oó]n|anmeld\w*|best[aä]tig\w*)\b",
        r"|التحقق|تحقق|تسجيل الدخول|تأكيد|الدخول"
    ))
    .unwrap()
});

/// Context meaning the number is something else (order, money, phone...).
pub static NEGATIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b(?:order|orders|dial[- ]?in|dial|meeting|webinar|conference|parkway|pkwy|street|avenue|suite|floor|invoice|receipt|tracking|track|",
        r"shipment|parcel|awb|waybill|reference|ref|total|subtotal|amount|price|paid|balance|phone|mobile|call|tel|fax|",
        r"hotline|zip|postal|post ?code|account (?:number|no)|acct|iban|swift|booking|reservation|pnr|flight|",
        r"seat|ticket|case|issue|card|ending|ends in|last 4|item|sku|model|serial|policy|membership|member|customer (?:id|number|no)|user ?id|",
        r"transaction|txn|employee|room|gate|terminal|version|build|release|chapter|episode|vol|points|miles|",
        r"commande|facture|pedido|factura|bestellung|rechnung|sendungsnummer|kundennummer)\b",
        r"|طلب|فاتورة|شحنة|تتبع|مبلغ|جنيه|ريال|درهم|هاتف|رصيد|نقاط|شارع"
    ))
    .unwrap()
});

/// Marketing words: a code near these is a promo code.
pub static PROMO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b(?:promo|promotional|coupon|discount|voucher|gift ?card|sale|off|checkout|offer|deal|deals|reward|",
        r"exclusive|save|savings|free shipping|redeem|cashback|r[eé]duction|descuento|rabatt|gutschein)\b|%",
        r"|خصم|كوبون|عرض|تخفيض|هدية|استرداد"
    ))
    .unwrap()
});

/// Dates, times and phone numbers: candidates overlapping these are dropped.
pub static NOT_CODE_SPANS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b\d{1,4}[/.-]\d{1,2}[/.-]\d{1,4}\b",
        r"|\b\d{1,2}:\d{2}(?::\d{2})?\b",
        r"|\b(?:jan|feb|mar|apr|may|jun|jul|aug|sep|sept|oct|nov|dec)[a-z]*\.?\s+\d{1,2}(?:st|nd|rd|th)?,?\s+\d{4}\b",
        r"|\b\d{1,2}\s+(?:jan|feb|mar|apr|may|jun|jul|aug|sep|sept|oct|nov|dec)[a-z]*\.?,?\s+\d{4}\b",
        r"|\d{1,2}\s+(?:يناير|فبراير|مارس|أبريل|ابريل|مايو|يونيو|يوليو|أغسطس|سبتمبر|أكتوبر|نوفمبر|ديسمبر)\s+\d{4}",
        r"|(?:\+|\b00)\d{1,3}[\s.-]?\(?\d{1,4}\)?(?:[\s.-]?\d{2,4}){2,4}",
        r"|\(\d{3}\)\s?\d{3}[\s.-]\d{4}|\b\d{3}[.-]\d{3}[.-]\d{4}\b|\b1[\s-]?800[\s-]?\d{3}[\s-]?\d{4}\b",
        r"|\b01[0125]\d{8}\b|\b\d{11,}\b",
        r"|(?-i:\b[A-Z]{2}\s\d{5}(?:-\d{4})?\b)"
    ))
    .unwrap()
});

/// Candidate tokens: optional short prefix like `G-`, then `123 456` /
/// `123-456` / `1234 5678`, plain 4-8 digits, or 4-8 uppercase alphanumerics.
pub static CANDIDATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:[A-Z]{1,3}-)?(\d{3,4}[ -]\d{3,4}|\d{4,8}|[A-Z0-9]{4,8})\b").unwrap()
});

/// WebOTP / origin-bound one-time code line: `@example.com #123456`.
pub static WEBOTP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*@([a-z0-9][a-z0-9.-]*\.[a-z]{2,})\s+#([A-Za-z0-9]{4,10})(?:\s+@[a-z0-9.-]+)?\s*$").unwrap()
});

/// "expires in 10 minutes" in several languages; group 1 = number, 2 = unit.
pub static EXPIRY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)(?:expires?|expiring|valid|good|active|available|expire|caduca|vence|v[aá]lid[oa]|g[uü]ltig|l[aä]uft)",
        r"[^.\n]{0,24}?(\d{1,3})\s*(min(?:ute)?s?|minutos?|minuten|hours?|horas?|heures?|stunden?|seconds?|secs?)",
        r"|(?:صالح|صالحة|ينتهي|تنتهي)[^.\n]{0,20}?(\d{1,3})\s*(دقائق|دقيقة|ساعة|ساعات|ثانية)"
    ))
    .unwrap()
});

pub static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:19|20)\d{2}$").unwrap());

/// Sender local parts typical of security mail.
pub static SECURITY_SENDER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:[^@]*(?:no-?reply|security|verify|verification|account|auth|login|signin|otp|identity|accounts|alert|notification)[^@]*)@").unwrap()
});

/// Words around a link that make it a one-click sign-in / verification link.
pub static LINK_TEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i)\b(?:sign[- ]?in|log[- ]?in|login|magic link|verify|confirm|activate|continue to|access your account)\b",
        r"|تسجيل الدخول|تأكيد|تفعيل|connexion|iniciar sesi[oó]n|anmelden|best[aä]tigen"
    ))
    .unwrap()
});

/// Anchor text that is unambiguously a one-click action.
pub static LINK_STRONG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:verify (?:my |your )?(?:email|account|address)|confirm (?:my |your )?(?:email|account|address)|activate (?:my |your )?account|magic link|sign in to|log in to|log in with)\b|تأكيد البريد|تفعيل الحساب").unwrap()
});

pub static LINK_URL_HINT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)magic|login|log-in|signin|sign-in|sign_in|/auth|verify|verification|confirm|passwordless|token=|otp|activate|session").unwrap()
});

pub static LINK_EXCLUDE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)unsubscribe|preferences|privacy|terms|/help|support|utm_|view.?in.?browser|facebook\.com|twitter\.com|x\.com/|instagram\.com|linkedin\.com|youtube\.com|tiktok\.com|play\.google\.com|apps\.apple\.com|reset|password").unwrap()
});
