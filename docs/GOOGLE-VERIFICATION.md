# Google Auth Platform — verification checklist

Owner-facing. The landing site in `site/` exists so Pulse Mail can fill
Google’s **Branding** fields (home page, privacy, terms, authorized domain)
and later submit **restricted-scope** verification. Nothing here submits
anything to Google; you still click the buttons in Cloud Console.

Consent-screen **app name must be `Pulse Mail`**. That is the string in
`src-tauri/tauri.conf.json` (`productName`, window `title`), the README, and
the OAuth setup doc. Do not use “Pulse”, “PulseMail”, or a GitHub handle —
Google checks that the name on the consent screen matches the product.

## 1. Ship the site

1. Merge the Pages PR to `main`.
2. In the GitHub repo: **Settings → Pages → Build and deployment → Source →
   GitHub Actions**. Save. This is the only step the workflow cannot do for
   you. Docs:
   [Configuring a publishing source](https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site).
3. Run **Actions → Deploy site to Pages → Run workflow** (or push a trivial
   change under `site/`). The first deploy after switching the source should
   succeed and print the URL.
4. Confirm these load without a login:

   | Page | URL |
   | --- | --- |
   | Home | https://abdallahsaber065.github.io/pulse-mail/ |
   | Privacy | https://abdallahsaber065.github.io/pulse-mail/privacy/ |
   | Terms | https://abdallahsaber065.github.io/pulse-mail/terms/ |

5. Optional but useful: GitHub **Settings → General → Website** =
   `https://abdallahsaber065.github.io/pulse-mail/` so the repo header
   matches the consent screen.

Until step 2, the workflow is present but Pages deploys fail.

## 2. Branding fields to paste

Google Auth Platform → **Branding**
(`https://console.cloud.google.com/auth/branding`).

| Field | Paste this |
| --- | --- |
| App name | `Pulse Mail` |
| User support email | your Google account (the one that owns the Cloud project) |
| App logo | `docs/pulse-mail-logo.png` (the neon envelope). Square PNG. |
| Application home page | `https://abdallahsaber065.github.io/pulse-mail/` |
| Privacy policy link | `https://abdallahsaber065.github.io/pulse-mail/privacy/` |
| Terms of service link | `https://abdallahsaber065.github.io/pulse-mail/terms/` |
| Authorized domains | `abdallahsaber065.github.io` |
| Developer contact email | same Google account (or a mailbox you actually read) |
| App domain | add the three URLs above **after** the authorized domain is saved |

Homepage rules Google will check
([brand verification](https://developers.google.com/identity/protocols/oauth2/production-readiness/brand-verification),
[homepage help](https://support.google.com/cloud/answer/13807376)):

- publicly reachable, not a login wall
- describes what the app does **and** why it wants Gmail data
- links the **same** privacy-policy URL as the Branding field
- hosted on a domain you have verified in Search Console

`github.io` is on the [Public Suffix List](https://publicsuffix.org/), so the
**top private domain** of this site is `abdallahsaber065.github.io`, not
`github.io`. Do not add `github.io` itself.

## 3. Verify the domain in Search Console

Google will not accept the Branding URLs until a Google account that is
**Owner or Editor** on the Cloud project has verified the authorized domain
in Search Console
([verify site ownership](https://support.google.com/webmasters/answer/9008080)).

You **cannot** use a Search Console **Domain** property (`abdallahsaber065.github.io`
without `https://`). That method needs a DNS TXT record, and you do not
control `github.io` DNS. Use a **URL-prefix** property.

### Recommended: HTML meta tag (already wired)

1. Open [Google Search Console](https://search.google.com/search-console).
2. Add property → **URL prefix** →
   `https://abdallahsaber065.github.io/pulse-mail/`
3. Choose **HTML tag**. Copy the `content="…"` value (not the whole tag).
4. Paste that value as the first non-comment line of
   [`site/google-site-verification.content`](../site/google-site-verification.content).
5. Commit, push to `main`, wait for **Deploy site to Pages**.
6. View source on the live home page and confirm
   `<meta name="google-site-verification" content="…">` is present.
7. Back in Search Console, click **Verify**.
8. Use **the same Google account** as a Cloud project Owner/Editor.

The assemble script (`scripts/assemble-site.sh`) injects the token at deploy
time. An empty file omits the meta tag so we never publish a dummy token.

### Fallback: HTML file

Search Console can instead give you a file named like `googleXXXXXXXX.html`.

1. Put that file in `site/` (repo path `site/googleXXXXXXXX.html`).
2. Push to `main`. It is served at
   `https://abdallahsaber065.github.io/pulse-mail/googleXXXXXXXX.html`.
3. Click **Verify**. Do not add a secret to that file; Google’s token is
   meant to be public.

### If Cloud Console still refuses the domain

Some reviews want verification of the host
`https://abdallahsaber065.github.io/` (no `/pulse-mail`). That URL 404s
unless you also have a **user** GitHub Pages site (a repo named
`abdallahsaber065.github.io`). If Branding rejects the project URL-prefix:

1. Create that user-site repo, put the HTML verification file at its root,
   enable Pages, verify `https://abdallahsaber065.github.io/` in Search
   Console, **or**
2. Reply to Trust & Safety that the product lives at the `/pulse-mail/`
   prefix and the meta tag is on that homepage.

Keep the Cloud project’s OAuth Branding Google account listed as a verified
owner of the Search Console property.

## 4. Scopes the app actually requests

From `SCOPE` in `src-tauri/src/auth/oauth.rs`:

```
https://mail.google.com/ openid profile
```

Declare **all three** on Auth Platform → **Data Access**. Google classifies
them when you add them
([OAuth scopes](https://developers.google.com/identity/protocols/oauth2/scopes),
[Gmail scopes](https://developers.google.com/workspace/gmail/api/auth/scopes)):

| Scope | What Pulse Mail uses it for | Google bucket |
| --- | --- | --- |
| `https://mail.google.com/` | Gmail REST (list/get/send/history/labels/trash) **and** IMAP XOAUTH2 so IDLE can push new mail | **Restricted** |
| `openid` | OpenID Connect; required to call userinfo | Non-sensitive (Sign-In) |
| `profile` (`https://www.googleapis.com/auth/userinfo.profile`) | Account display name + photo after sign-in | Non-sensitive (Sign-In) |

`https://mail.google.com/` is the Gmail IMAP/SMTP/full-mail scope. Google’s
Gmail API docs say: request it only if the app must **immediately and
permanently delete** mail, bypassing Trash; otherwise use a narrower Gmail
API scope. Pulse Mail’s UI trash path is Gmail `threads.trash` (mail goes to
Trash). The **reason we still need this scope** is IMAP IDLE (XOAUTH2).
Google’s IMAP documentation states that the scope for IMAP is
`https://mail.google.com/`. Narrower Gmail API scopes cannot authorize IMAP.
Write that in the justification — do not claim we EXPUNGE-delete to justify
the scope.

Approved Gmail use case we match: *“Built-in and web email clients that
allow users to compose, send, read, and process email via a user interface.”*
([Workspace user-data policy](https://developers.google.com/workspace/workspace-api-user-data-developer-policy)).

## 5. What “restricted” means in plain words

Until Google **verifies** this Cloud project:

- Sign-in shows **“Google hasn’t verified this app”**.
- Unverified apps that ask for restricted Gmail access are capped at about
  **100 users**.
- Publishing status must stay **In production** or Google expires refresh
  tokens after 7 days (Testing). Verification is a separate step from
  “In production”.

To go past that cap you complete two reviews
([FAQ](https://support.google.com/cloud/answer/13463817)):

1. **Brand verification** (about 2–3 business days if the site is in order):
   name, logo, homepage, privacy, terms, domain ownership.
2. **Restricted-scope / data-access verification** (often weeks): scope
   justifications, a YouTube demo, Limited Use compliance, and — if they
   decide you have a server in the middle — a **CASA security assessment**.

Brand verification is not Gmail-scope approval. You can publish branding
and still have the unverified-app screen until data-access verification
finishes.

## 6. CASA (security assessment)

Google requires an annual CASA assessment from a Google-empanelled assessor
when an app accesses restricted Google user data **from or through a
third-party server**
([restricted-scope verification](https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification),
[security assessment help](https://support.google.com/cloud/answer/13465431)).

Pulse Mail does **not** operate a backend. Mail and tokens stay on the
user’s computer; the only extra host is this repo’s GitHub Releases for
self-update (installers, not mail). That is the textbook **local / native
email client**.

**Do not assume you are exempt until Trust & Safety say so.** In the
verification form, describe it as a local desktop email client that does
not transmit restricted-scope data to developer servers. If they still
require CASA: you pay an independent assessor (Google does not collect the
fee); the letter is typically valid 12 months; adding another restricted
scope can trigger a re-assessment.

If they never ask for CASA, you still owe brand verification, the demo
video, and the justifications below.

## 7. YouTube demo video

Required for sensitive/restricted scopes
([demo video help](https://support.google.com/cloud/answer/13804565),
[restricted-scope steps](https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification#how-to-submit-your-app-for-brand-verification)).

Record the **installed** app (not `pnpm dev` mock mode):

1. OAuth grant in **English** (consent-screen language control, bottom-left).
2. Consent screen showing the app name **Pulse Mail**.
3. Browser address bar on the consent page showing this project’s **OAuth
   client ID**.
4. After grant: inbox sync, open a thread, send or save a draft, archive,
   IMAP-driven toast if you can show it (or say in narration that IDLE is
   why `https://mail.google.com/` is requested).
5. Account switcher showing the Google name/photo (`profile`).
6. If you have more than one OAuth client in the project, show each. Delete
   leftover clients before submitting.

Upload to YouTube Studio, visibility **Unlisted**, paste the link in the
verification form.

## 8. Scope justifications (draft)

Paste and edit. Keep them specific to the running binary.

**`https://mail.google.com/`**

> Pulse Mail is a desktop Gmail client (Windows daily driver; experimental
> macOS/Linux). Users read, search, send, reply, archive, star, label, and
> trash mail in the app. New-mail notifications use IMAP IDLE over XOAUTH2
> as a doorbell, then Gmail `history.list` for the cheap delta. Google’s
> IMAP documentation requires the `https://mail.google.com/` scope for
> IMAP; narrower Gmail API scopes cannot authenticate that connection. We
> do not use IMAP to silently bypass Trash: the UI delete path calls Gmail
> `users.threads.trash`. We do not export mail to our servers — there is
> no Pulse Mail server. Data stays in a local SQLite cache and the OS
> credential store. This matches the approved use case “built-in email
> client.” Narrower scopes (`gmail.readonly`, `gmail.send`, `gmail.modify`)
> would drop IMAP push, which is a core feature.

**`openid` + `profile`**

> After the user finishes the browser sign-in, Pulse Mail calls Google’s
> OpenID Connect userinfo endpoint for the account’s display name and
> profile photo so the in-app account menu can show who is signed in. We
> do not use Google Sign-In as a login for some other product. Photo bytes
> are size-capped and stored locally as a data URL. Users who only granted
> the old Gmail-only scope keep working until they reconnect.

## 9. Limited Use (privacy policy)

The privacy page already includes the required statement, with a link to
https://developers.google.com/terms/api-services-user-data-policy

Do not add features that ship Google user data to ads, brokers, or model
training. Do not add a backend that stores mail without revisiting this
doc, CASA, and the privacy policy.

## 10. Submit order

1. Pages live, Search Console verified, Branding URLs saved.
2. Audience = **External**, publishing status **In production**.
3. Branding → **Verify branding** → **Publish branding**.
4. OAuth Verification Center → Data access → declare the three scopes,
   justifications, demo video, application type = email client.
5. Watch the developer-contact mailbox. Reviews pause if you don’t reply.

Personal use (you + a few people you know) does **not** require this
process; the unverified warning and ~100-user cap stay
([when verification is not needed](https://support.google.com/cloud/answer/13464323)).
Submit when you want strangers to sign in without the warning.
