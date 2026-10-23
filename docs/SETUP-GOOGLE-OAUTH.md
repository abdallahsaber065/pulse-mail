# Set up your Google OAuth client (one time, ~10 minutes)

Pulse Mail signs in with **your own** Google Cloud OAuth client, so no third
party ever sees your mail or tokens. You only do this once.

> You need a Google account that can use Google Cloud (free; no billing account
> is needed for the Gmail API).

## 1. Create a project

1. Open <https://console.cloud.google.com/projectcreate>.
2. Project name: `Pulse Mail` → **Create**. Make sure it's selected in the top bar.

## 2. Turn on the Gmail API

1. Open <https://console.cloud.google.com/apis/library/gmail.googleapis.com>.
2. Click **Enable**.

## 3. Configure the consent screen (Google Auth Platform)

1. Open <https://console.cloud.google.com/auth/overview> → **Get started**.
2. **App information:** app name `Pulse Mail`, user support email = your Gmail.
3. **Audience:** **External**.
4. **Contact information:** your email → agree to the policy → **Create**.
5. Go to **Data Access** → **Add or remove scopes** → under "Manually add
   scopes" add `https://mail.google.com/` and `openid`, and select the basic
   `https://www.googleapis.com/auth/userinfo.profile` scope → **Add to table**
   → **Update** → **Save**. The sign-in request uses its standard `profile`
   alias. This supplies your Google name and photo; Gmail access supplies
   your mail. See [Google's scope reference](https://developers.google.com/identity/protocols/oauth2/scopes).

If already signed in with Gmail-only access, use **Settings → Accounts →
your account → Reconnect Google** once and grant profile access. Your cached
mail and settings stay in place. A name you entered in Settings takes
priority over Google's name; clear it to use Google's default again.

## 4. Publish the app ("In production"): important

1. Go to **Audience** → **Publish app** → **Confirm**.
   Status should read **In production**.

Why: while an app is in **Testing**, Google expires refresh tokens after
**7 days**, so a 24/7 client would sign you out every week. "In production"
without verification is fine for personal use: Google caps unverified apps
with restricted scopes at 100 users, and you are one. You don't need to
submit for verification.

## 5. Create the Desktop client

1. Go to **Clients** (<https://console.cloud.google.com/auth/clients>) → **Create client**.
2. Application type: **Desktop app**. Name: `Pulse Mail (Windows)` → **Create**.
3. In the dialog click **Download JSON**. You can't see the secret again
   later; if you lose the file, add a new secret from the client page.

## 6. Put the file where Pulse Mail looks for it

You can save the downloaded JSON as `google-oauth-client.json` in either location:

### Option A: Directly in the Project Root (Easiest for Development)
Save it directly to the root of the `pulse-mail` repository as:
```
./google-oauth-client.json
```
*(Note: `.gitignore` strictly protects `google-oauth-client*.json` so git will never track or commit it).*

### Option B: System User AppData / Config Directory
Save the file as:
```
%APPDATA%\com.abdallah.pulsemail\google-oauth-client.json
```
(PowerShell snippet if downloaded to Downloads):
```powershell
$dir = "$env:APPDATA\com.abdallah.pulsemail"
New-Item -ItemType Directory -Force $dir | Out-Null
Move-Item "$env:USERPROFILE\Downloads\client_secret_*.json" "$dir\google-oauth-client.json"
icacls "$dir\google-oauth-client.json" /inheritance:r /grant:r "${env:USERNAME}:(R,W)" "SYSTEM:(F)" | Out-Null
```

Alternative: set the `PULSE_MAIL_OAUTH_CLIENT` environment variable to the path of your JSON file.

## 7. Sign in

1. Start Pulse Mail (`pnpm tauri dev`, or the installed app).
2. Click **Sign in with Google**. Your browser opens Google's page.
3. You'll see **"Google hasn't verified this app"**. That's expected for
   your own unverified client: **Advanced** → **Go to Pulse Mail (unsafe)**.
4. Allow **Read, compose, send and permanently delete all your email from Gmail**
   (this is the `https://mail.google.com/` scope; IMAP push needs it).
5. The tab says "You're signed in". Close it. Pulse Mail starts syncing.

Your refresh token is now in **Windows Credential Manager** (Control
Panel → Credential Manager → Windows Credentials → `com.abdallah.pulsemail`).
Nothing secret is written to disk by Pulse Mail.

## Troubleshooting

| Symptom | Fix |
| --- | --- |
| "isn't connected to a Google OAuth client yet" | File missing or misnamed. Check the path from step 6. |
| "The Google OAuth client file isn't valid" | You downloaded a **Web** client or edited the file. Create a **Desktop app** client. |
| `Error 403: access_denied` in the browser | The app is still in Testing and your account isn't a test user. Publish it (step 4). |
| Signed out after about a week | App still in Testing. Publish it (step 4), then sign in again. |
| "Your Google sign-in expired" | You revoked access at <https://myaccount.google.com/permissions> or changed your password. Sign in again. |
| No toasts in `pnpm tauri dev` | Dev builds borrow PowerShell's toast identity; check Windows notification settings for PowerShell, or use the installed build. |

## Revoking

- In the app: click your avatar → sign out (revokes and deletes local data).
- Or at <https://myaccount.google.com/permissions> → Pulse Mail → Remove access.
