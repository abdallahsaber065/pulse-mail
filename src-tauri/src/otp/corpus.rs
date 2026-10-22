//! Labeled corpus used to tune the OTP weights and to guard against
//! regressions. Samples are synthetic but follow the real formats of the
//! named services (wording, layout, code shape). Run with
//! `cargo test otp::corpus -- --nocapture` to print precision/recall.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Want {
    Code(&'static str),
    Link,
    Nothing,
}
use Want::*;

struct Sample {
    name: &'static str,
    from: &'static str,
    subject: &'static str,
    text: &'static str,
    html: Option<&'static str>,
    promo: bool,
    want: Want,
}

const fn s(name: &'static str, from: &'static str, subject: &'static str, text: &'static str, want: Want) -> Sample {
    Sample { name, from, subject, text, html: None, promo: false, want }
}
const fn h(name: &'static str, from: &'static str, subject: &'static str, html: &'static str, want: Want) -> Sample {
    Sample { name, from, subject, text: "", html: Some(html), promo: false, want }
}
const fn p(name: &'static str, subject: &'static str, text: &'static str, want: Want) -> Sample {
    Sample { name, from: "deals@shop.example", subject, text, html: None, promo: true, want }
}

const SAMPLES: &[Sample] = &[
    // ---- Codes: global services ----
    s("google recovery", "noreply@accounts.google.com", "Google Verification Code",
      "Google\n\nVerify your email address\n\nGoogle received a request to use you@example.com as a recovery email for Google Account abc@gmail.com.\n\nUse this code to finish setting up this recovery email:\n\n582913\n\nThis code will expire in 24 hours.\n\nIf you don't recognize abc@gmail.com, you can safely ignore this email.",
      Code("582913")),
    s("google G- prefix", "noreply@google.com", "G-582913 is your Google verification code", "G-582913 is your Google verification code.", Code("582913")),
    s("microsoft security code", "account-security-noreply@accountprotection.microsoft.com", "Microsoft account security code",
      "Microsoft account\nSecurity code\n\nPlease use the following security code for the Microsoft account ab***@gmail.com.\n\nSecurity code: 739104\n\nIf you don't recognize the Microsoft account ab***@gmail.com, you can click here to remove your email address from that account.\n\nThanks,\nThe Microsoft account team",
      Code("739104")),
    s("microsoft 4-digit", "account-security-noreply@accountprotection.microsoft.com", "Microsoft account security code",
      "Please use the following security code for the Microsoft account.\n\nSecurity code: 7391", Code("7391")),
    s("apple id", "appleid@id.apple.com", "Verify your Apple ID email address",
      "Dear Abdallah,\n\nYou have selected this email address as your new Apple ID. To verify this email address belongs to you, enter the code below on the email verification page:\n\n418265\n\nThis code will expire three hours after this email was sent.\n\nApple ID Support",
      Code("418265")),
    s("github device", "noreply@github.com", "[GitHub] Please verify your device",
      "Hey abdallah!\n\nA sign in attempt requires further verification because we did not recognize your device. To complete the sign in, enter the verification code on the unrecognized device.\n\nDevice: Chrome on Windows\nVerification code: 731590\n\nIf you did not attempt to sign in to your account, your password may be compromised. Visit https://github.com/settings/security to create a new, strong password for your GitHub account.",
      Code("731590")),
    s("github launch code", "noreply@github.com", "[GitHub] Your GitHub launch code",
      "Here is your GitHub launch code: 84213590\n\nContinue signing up for GitHub by entering the code above.", Code("84213590")),
    s("amazon identity", "account-update@amazon.com", "Amazon password assistance",
      "To verify your identity, please use the following code:\n\n204817\n\nAmazon takes your account security very seriously. Amazon will never email you and ask you to disclose or verify your Amazon password, credit card, or banking account number.",
      Code("204817")),
    s("amazon split", "account-update@amazon.com", "Your Amazon verification code",
      "Sign-in attempt\nEnter this code to sign in: 552 019\nDon't share this code with anyone.", Code("552019")),
    s("paypal", "service@paypal.com", "Your PayPal security code",
      "PayPal: Your security code is 483920. Your code expires in 10 minutes. Don't share your code with anyone.", Code("483920")),
    s("steam guard", "noreply@steampowered.com", "Your Steam account: Access from new web or mobile device",
      "Dear abdallah,\n\nHere is the Steam Guard code you need to login to account abdallah:\n\nF7K2Q\n\nThis email was generated because of a login attempt from a web or mobile device located at Cairo, EG.",
      Code("F7K2Q")),
    s("facebook reset", "security@facebookmail.com", "48213921 is your Facebook account recovery code",
      "Hi Abdallah,\nWe received a request to reset your Facebook password.\nEnter the following password reset code:\n\n48213921", Code("48213921")),
    s("instagram", "security@mail.instagram.com", "550912 is your Instagram code", "550912 is your Instagram code. Don't share it.", Code("550912")),
    s("x verification", "verify@x.com", "Your X verification code is 624019",
      "Confirm your email address\nThere's one quick step you need to complete before creating your X account.\nPlease enter this verification code to get started on X:\n624019\nVerification codes expire after two hours.",
      Code("624019")),
    s("whatsapp", "noreply@whatsapp.com", "WhatsApp code",
      "Your WhatsApp code: 482-913\nYou can also tap on this link to verify your phone: v.whatsapp.com/482913\nDon't share this code with others", Code("482913")),
    s("netflix 4-digit", "info@account.netflix.com", "Your Netflix sign-in code",
      "Enter this code to sign in\n\n2841\n\nThis code will expire in 15 minutes.", Code("2841")),
    s("uber", "noreply@uber.com", "Uber", "Your Uber code is 4821. Never share this code.", Code("4821")),
    s("binance", "do-not-reply@ses.binance.com", "[Binance] Withdrawal verification",
      "[Binance] Withdrawal verification code: 482913. The code is valid for 30 minutes. Never share it.", Code("482913")),
    s("zoom", "no-reply@zoom.us", "Zoom verification", "Your Zoom verification code is 391820. It expires in 10 minutes.", Code("391820")),
    s("dropbox", "no-reply@dropbox.com", "Dropbox security code", "Your Dropbox security code is: 482913", Code("482913")),
    s("linkedin pin", "security-noreply@linkedin.com", "Abdallah, here's your PIN 739104",
      "Hi Abdallah,\nPlease use this verification code to complete your sign in: 739104\nIf you didn't try to sign in, change your password.", Code("739104")),
    s("telegram", "noreply@telegram.org", "Telegram login",
      "Your login code: 48213. Do not give this code to anyone, even if they say they are from Telegram!", Code("48213")),
    s("samsung brackets", "sa.noreply@samsung-mail.com", "Samsung account", "Your Samsung account verification code is [482913].", Code("482913")),
    s("epic alnum", "help@acct.epicgames.com", "Your Epic Games security code", "Your Epic Games security code is: 7K2PQ9", Code("7K2PQ9")),
    s("twitch", "no-reply@twitch.tv", "Twitch", "Your Twitch verification code: 482913", Code("482913")),
    s("okta", "noreply@okta.com", "One-time verification code",
      "Your one-time verification code is 482913. This code expires in 5 minutes.", Code("482913")),
    s("shopify 4-4", "mailer@shopify.com", "Shopify login code", "Your Shopify login code is 4821-5932", Code("48215932")),
    s("slack split", "feedback@slack.com", "Slack confirmation code: 482-913",
      "Confirm your email address\nYour confirmation code is below — enter it in your open browser window and we'll help you get signed in.\n\n482-913\n\nIf you didn't request this email, there's nothing to worry about.",
      Code("482913")),
    s("subject only", "no-reply@service.example", "Your code: 739104", "Use it to sign in.", Code("739104")),
    s("webotp line", "no-reply@example.com", "Sign in", "Your code is 482913.\n\n@example.com #482913", Code("482913")),
    s("zero width", "no-reply@example.com", "Verification", "Your verification code is 4\u{200b}8\u{200b}2\u{200b}9\u{200b}1\u{200b}3", Code("482913")),
    s("full width digits", "no-reply@example.jp", "認証コード", "認証コード: ４８２９１３", Code("482913")),
    s("expires on date", "no-reply@example.com", "Verify", "Your verification code is 482913. It expires on 21 Oct 2026. Total charged: $12.50", Code("482913")),
    // ---- Codes: banks and Egyptian services ----
    s("cib bank otp", "alerts@cibeg.com", "OTP",
      "Dear Customer, your OTP for transaction of EGP 1,250.00 at AMAZON is 482913. Valid for 5 minutes. Do not share it with anyone.", Code("482913")),
    s("nbe arabic", "nbe@nbe.com.eg", "رمز التحقق",
      "عميلنا العزيز، رمز التحقق الخاص بك هو 482913 لعملية شراء بمبلغ 250.00 جنيه. لا تشارك هذا الرمز مع أحد.", Code("482913")),
    s("vodafone arabic digits", "noreply@vodafone.com.eg", "كود التفعيل", "كود التفعيل الخاص بك هو ٤٨٢٩ صالح لمدة ٥ دقائق - فودافون", Code("4829")),
    s("ana vodafone", "noreply@vodafone.com.eg", "أنا فودافون", "رمز التحقق لتطبيق أنا فودافون هو: 739104", Code("739104")),
    s("e& egypt", "noreply@eand.com.eg", "e& verification", "Your e& verification code is 5821", Code("5821")),
    s("fawry arabic", "no-reply@fawry.com", "فوري", "كود التحقق الخاص بفوري هو 830192، لا تشاركه مع أي شخص.", Code("830192")),
    s("instapay", "noreply@instapay.eg", "InstaPay", "Your InstaPay OTP is 482913. Never share it with anyone, including bank staff.", Code("482913")),
    s("talabat", "no-reply@talabat.com", "talabat", "Your talabat verification code is 2841", Code("2841")),
    s("stc arabic digits", "noreply@stc.com.sa", "رمز التحقق", "رمز التحقق: ٧٣٩١٠٤", Code("739104")),
    s("arabic nbsp", "noreply@bank.example", "رمز التحقق", "مرحبا،\u{a0}رمز\u{a0}التحقق:\u{a0}558812\u{a0}صالح لمدة ١٠ دقائق", Code("558812")),
    s("arabic otp phrase", "noreply@bank.example", "كلمة المرور لمرة واحدة", "كلمة المرور لمرة واحدة هي 902341 وتنتهي خلال 3 دقائق", Code("902341")),
    // ---- Codes: other languages ----
    s("french", "noreply@exemple.fr", "Code de vérification", "Votre code de vérification est : 482 913. Il expire dans 10 minutes.", Code("482913")),
    s("spanish", "noreply@ejemplo.es", "Código", "Tu código de verificación es 730194. Caduca en 10 minutos.", Code("730194")),
    s("german", "noreply@beispiel.de", "Ihr Bestätigungscode", "Ihr Bestätigungscode lautet: 482913. Der Code ist 10 Minuten gültig.", Code("482913")),
    // ---- Codes: HTML-only ----
    h("html big cell", "no-reply@accounts.example.com", "Your one-time code",
      "<html><body><table><tr><td>Use this one-time code to sign in</td></tr><tr><td style=\"font-size:32px;letter-spacing:8px;font-weight:bold\">482913</td></tr><tr><td>It expires in 10 minutes. Order history: 2026.</td></tr></table><p>&copy; 2026 Example Inc, 1600 Main Street</p></body></html>",
      Code("482913")),
    h("html bold among numbers", "security@bank.example", "Sign-in verification",
      "<p>Hello,</p><p>We noticed a sign-in from Windows on 21/10/2026 at 10:30.</p><p>Verify it's you with: <strong>739104</strong></p><p>Call 19777 if this wasn't you.</p>",
      Code("739104")),
    // ---- Links ----
    h("slack magic link", "feedback@slack.com", "Slack magic link",
      "<p>Click the button below to sign in.</p><a href=\"https://app.slack.com/t/acme/login/z-app-123-abcdef1234567890abcdef\">Sign in to Slack</a><a href=\"https://slack.com/help\">Help</a>",
      Link),
    h("medium sign in", "noreply@medium.com", "Sign in to Medium",
      "<p>Click below to sign in to your Medium account. This link will expire in 15 minutes.</p><a href=\"https://medium.com/m/callback/email?token=8f2b1c3d4e5f6a7b8c9d0e1f&amp;operation=login\">Sign in to Medium</a><a href=\"https://medium.com/unsubscribe?u=1\">Unsubscribe</a>",
      Link),
    h("discord verify email", "noreply@discord.com", "Verify Email Address for Discord",
      "<p>Thanks for registering for an account on Discord! Before we get started, we just need to confirm that this is you.</p><a href=\"https://click.discord.com/ls/click?upn=AbCdEf1234567890xyzLongToken\">Verify Email</a>",
      Link),
    s("notion text link", "notify@mail.notion.so", "Your Notion login link",
      "Click here to log in with this magic link: https://www.notion.so/loginwithemail?state=abc123def456ghi789jkl&token=xyz\n\nIf you didn't try to login, you can safely ignore this email.", Link),
    h("vercel verify", "notifications@vercel.com", "Vercel Login Verification",
      "<p>A sign in attempt requires verification.</p><a href=\"https://vercel.com/confirm?email=a%40b.com&amp;token=abcdef1234567890abcdef\">VERIFY</a>",
      Link),
    // ---- Negatives ----
    s("amazon shipped", "shipment-tracking@amazon.com", "Your Amazon.com order has shipped",
      "Your order #112-4829135-1234567 has shipped. Arriving Thursday.\nTrack package\nOrder total: $49.99", Nothing),
    s("uber receipt", "noreply@uber.com", "Your Thursday trip with Uber",
      "Thanks for riding. Total EGP 125.50. Trip on 21 Oct 2026 at 10:30. Invoice number 48291", Nothing),
    s("rust newsletter", "twir@rust-lang.org", "This Week in Rust 612",
      "This week in Rust 612. 25 new crates, 1500 contributors. Read more at the blog. Learn to write better code with 4821 examples.", Nothing),
    s("promo code", "deals@store.example", "Big sale", "Use promo code SAVE20 for 20% off everything", Nothing),
    s("arabic promo", "deals@store.example", "عرض خاص", "استخدم كود EID25 واحصل على خصم 25% على كل الطلبات", Nothing),
    s("aramex awb", "noreply@aramex.com", "Out for delivery",
      "Your shipment with AWB number 48291035 is out for delivery. Call 19911 for help.", Nothing),
    s("booking code", "noreply@egyptair.com", "Booking confirmed",
      "Your booking confirmation code is K7X2PQ. Flight MS777 departs 22 Oct 2026 10:30.", Nothing),
    s("meeting pin", "calendar@example.com", "Invitation: Sync",
      "Meeting at 10:30, Room 4021, dial-in +1 415 555 0199, PIN 482913#", Nothing),
    s("bank credit", "alerts@bank.example", "Account credited",
      "Your account ending 4821 was credited with EGP 5,000.00 on 21/10/2026. Available balance EGP 12,345.67.", Nothing),
    s("pr review", "notifications@github.com", "[org/repo] Fix code extraction (#4821)",
      "Requested your review on PR #4821: Fix code extraction. 12 files changed.", Nothing),
    s("password changed", "security@example.com", "Your password was changed",
      "Your password was changed on Oct 21, 2026 at 10:30 PM. If this wasn't you, secure your account.", Nothing),
    s("new sign-in ip", "no-reply@accounts.google.com", "Security alert",
      "New sign-in to your account from 197.45.12.80 on Windows. If this was you, no action needed.", Nothing),
    s("postal code", "orders@shop.example", "Address saved", "Postal code: 11511. Your address was saved.", Nothing),
    s("tracking code", "noreply@dhl.com", "Shipment", "Tracking code: 48213921. Expected delivery Friday.", Nothing),
    s("2fa enabled", "noreply@github.com", "[GitHub] Two-factor authentication enabled",
      "You enabled two-factor authentication on Oct 21, 2026. Recovery codes were generated; store them safely.", Nothing),
    s("call support", "support@service.example", "Request received",
      "We received your request. If you didn't request this code, call 19777.", Nothing),
    s("footer address", "news@devs.example", "Learn to code", "Learn how to code this weekend.\n© 2026 Google LLC, 1600 Amphitheatre Parkway, Mountain View, CA 94043", Nothing),
    s("arabic order", "orders@shop.example", "تم تأكيد طلبك", "تم تأكيد طلبك رقم 482913 بقيمة 350 جنيه. سيصلك خلال 3 أيام.", Nothing),
    h("arabic order bold", "orders@shop.example", "تأكيد الطلب", "<p>تم تأكيد طلبك رقم <b>482913</b> بقيمة 350 جنيه.</p>", Nothing),
    h("coupon html", "deals@shop.example", "A gift for you",
      "<p>Use this code at checkout:</p><div style=\"font-size:28px;font-weight:bold\">SAVE20</div>", Nothing),
    p("promotions exclusive", "Your exclusive code inside", "Your exclusive code: VIP482 — valid this week only.", Nothing),
    s("zoom meeting passcode", "no-reply@zoom.us", "Zoom meeting invitation",
      "Join Zoom Meeting\nMeeting ID: 482 9135 1234\nPasscode: 739104", Nothing),
    s("release build", "releases@pulse.example", "Pulse 2.4.1 released", "Pulse 2.4.1 released. Build 48213. Source code on GitHub.", Nothing),
    s("social", "notification@facebookmail.com", "Ahmed and 12 others", "Ahmed and 12 others liked your post.", Nothing),
    s("arabic debit", "alerts@bank.example", "خصم من حسابك", "تم خصم مبلغ 1,250.00 جنيه من حسابك المنتهي بـ 4821 بتاريخ 21/10/2026", Nothing),
    h("password reset link", "no-reply@service.example", "Reset your password",
      "<p>Click to reset your password.</p><a href=\"https://service.example/reset-password?token=abcdef1234567890abcdef\">Reset password</a>", Nothing),
    h("newsletter links", "news@blog.example", "Weekly digest",
      "<p>Top stories this week.</p><a href=\"https://click.blog.example/abc1234567890defghij\">Read more</a><a href=\"https://blog.example/unsubscribe\">Unsubscribe</a>", Nothing),
    Sample { name: "promo login link", from: "deals@shop.example", subject: "Log in to claim your reward",
      text: "", html: Some("<a href=\"https://shop.example/login?token=abcdef1234567890abcdef\">Log in</a>"), promo: true, want: Nothing },
    s("egypt phone", "support@service.example", "كود", "لأي استفسار عن الكود اتصل بنا على 01012345678", Nothing),
    s("hex color", "design@studio.example", "Brand code", "Brand color #A1B2C3 for your code snippet.", Nothing),
    s("price with year-like", "sales@store.example", "Pricing", "Plans start at 1999 EGP. Use code at signup.", Nothing),
    s("gcp project id", "CloudPlatform-noreply@google.com", "Your project third-pen-350207 has been shut down",
      "Your Google Cloud project third-pen-350207 has been shut down. To restore it, visit the console. Verification of ownership is not required.", Nothing),
    s("card last 4", "noreply@ses.redotpay.com", "Your card 5195 has been successfully upgraded!",
      "Your card ending 5195 has been upgraded. Use your PIN as usual.", Nothing),
    s("order code words", "orders@shop.example", "Your order", "Order code: 482913. We'll let you know when it ships.", Nothing),
];


/// Written after tuning and never tuned against: an honest generalization check.
const HOLDOUT: &[Sample] = &[
    s("ho reddit", "noreply@reddit.com", "Your Reddit verification code", "Use the code below to verify your email.\n\n719402\n\nThis code expires in 30 minutes.", Code("719402")),
    s("ho spotify", "no-reply@spotify.com", "Your Spotify login code", "Here's your 6-digit login code: 302918", Code("302918")),
    s("ho tiktok", "no-reply@account.tiktok.com", "882031 is your verification code", "To verify your account, enter this code in TikTok: 882031", Code("882031")),
    s("ho yahoo", "no-reply@cc.yahoo-inc.com", "Your Yahoo verification code", "Use verification code 6039 to sign in to your account.", Code("6039")),
    s("ho adobe", "message@adobe.com", "Verification code", "Your Adobe verification code is 418 002. Do not share it.", Code("418002")),
    s("ho banque misr arabic", "info@banquemisr.com", "رمز", "الرقم السري المؤقت الخاص بك هو ٣٩٢٠١٨ ولا تشاركه مع أحد", Code("392018")),
    s("ho orange egypt", "noreply@orange.eg", "My Orange", "Your My Orange verification code is 4120. Valid for 3 minutes.", Code("4120")),
    s("ho careem arabic", "noreply@careem.com", "كريم", "كود الدخول الخاص بك هو 7741", Code("7741")),
    s("ho valu", "otp@valu.com.eg", "valU", "Dear client, 562019 is your valU OTP. It expires in 2 minutes.", Code("562019")),
    s("ho atlassian", "noreply@id.atlassian.com", "Verify your email for Atlassian", "Your verification code is:\n\nK7Q2-9XMP\n\nThis code expires in 10 minutes.", Code("K7Q29XMP")),
    s("ho french bank", "securite@banque.fr", "Code de sécurité", "Code de sécurité : 284 019. Ne le communiquez à personne.", Code("284019")),
    s("ho german tan", "noreply@bank.de", "Ihre TAN", "Ihre TAN für den Auftrag lautet 482019.", Code("482019")),
    h("ho html code tag", "no-reply@app.example", "Sign in to App", "<p>Enter this code to finish signing in:</p><p><code>631904</code></p><p>Sent on 21 Oct 2026.</p>", Code("631904")),
    h("ho figma link", "no-reply@figma.com", "Log in to Figma", "<p>Click the link below to log in.</p><a href=\"https://www.figma.com/email/login?token=QWERTYUIOP1234567890\">Log in to Figma</a>", Link),
    s("ho noon order", "no-reply@noon.com", "Order confirmed", "Your order N4821930 is confirmed. Total: EGP 1,299.00. Delivery on 24 Oct.", Nothing),
    s("ho invoice", "billing@saas.example", "Invoice 2026-1043", "Invoice #10432 for October. Amount due $29.00. Payment code is auto-charged.", Nothing),
    s("ho bosta tracking", "noreply@bosta.co", "شحنتك في الطريق", "رقم تتبع الشحنة 48213907، المندوب سيتصل بك على 01001234567", Nothing),
    s("ho newsletter codes", "news@dev.example", "5 code smells", "5 code smells to fix in 2026: 1) long methods 2) 3000-line files", Nothing),
    p("ho promo welcome", "Welcome gift", "Welcome! Here is your code WELCOME10 for 10% off your first order.", Nothing),
    s("ho calendar", "calendar-notification@google.com", "Reminder: Standup", "Standup at 09:30 - 09:45 (EET). Join with Google Meet: meet.google.com/abc-defg-hij. Join by phone +20 2 2598 1234 PIN: 482019#", Nothing),
];

fn run(sm: &Sample) -> Option<Detection> {
    let text = if sm.text.is_empty() { sm.html.map(html_to_text).unwrap_or_default() } else { sm.text.to_string() };
    detect(&Input { subject: sm.subject, text: &text, html: sm.html, from: sm.from, promotional: sm.promo })
}

/// (tp, fp, fn, tn, failures)
fn score(set: &[Sample]) -> (u32, u32, u32, u32, Vec<String>) {
    let (mut tp, mut fp, mut fn_, mut tn) = (0, 0, 0, 0);
    let mut failures = Vec::new();
    for sm in set {
        let got = run(sm);
        let ok = match (sm.want, &got) {
            (Code(c), Some(d)) => d.kind == Kind::Code && d.value == c,
            (Link, Some(d)) => d.kind == Kind::Link,
            (Nothing, None) => true,
            _ => false,
        };
        match (sm.want, ok) {
            (Nothing, true) => tn += 1,
            (Nothing, false) => fp += 1,
            (_, true) => tp += 1,
            (_, false) => {
                fn_ += 1;
                if got.is_some() {
                    fp += 1;
                }
            }
        }
        if !ok {
            failures.push(format!("{}: want {:?}, got {:?}", sm.name, sm.want, got));
        }
    }
    (tp, fp, fn_, tn, failures)
}

#[test]
fn corpus_precision_recall() {
    let (tp, fp, fn_, tn, failures) = score(SAMPLES);
    let precision = tp as f64 / (tp + fp).max(1) as f64;
    let recall = tp as f64 / (tp + fn_).max(1) as f64;
    println!(
        "OTP corpus: {} samples, tp={tp} fp={fp} fn={fn_} tn={tn}, precision={:.3}, recall={:.3}",
        SAMPLES.len(),
        precision,
        recall
    );
    for f in &failures {
        println!("  miss: {f}");
    }
    let (htp, hfp, hfn, htn, hfail) = score(HOLDOUT);
    println!(
        "OTP holdout: {} samples, tp={htp} fp={hfp} fn={hfn} tn={htn}, precision={:.3}, recall={:.3}",
        HOLDOUT.len(),
        htp as f64 / (htp + hfp).max(1) as f64,
        htp as f64 / (htp + hfn).max(1) as f64
    );
    for f in &hfail {
        println!("  holdout miss: {f}");
    }
    assert!(SAMPLES.len() >= 80, "corpus too small");
    assert!(precision >= 0.97 && recall >= 0.95, "precision {precision:.3} recall {recall:.3}: {failures:#?}");
}

#[test]
fn expiry_is_read() {
    let d = run(&SAMPLES[9]).unwrap(); // paypal
    assert_eq!(d.expires_min, Some(10));
    let d = detect(&Input { subject: "رمز", text: "رمز التحقق 482913 صالح لمدة ٥ دقائق", ..Default::default() }).unwrap();
    assert_eq!(d.expires_min, Some(5));
}
