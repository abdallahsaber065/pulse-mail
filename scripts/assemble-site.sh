#!/usr/bin/env bash
# Build a publishable copy of the landing site into _site/ (gitignored).
# Copies screenshots/logo from docs/ and injects the Search Console token
# from site/google-site-verification.content when the owner has filled it in.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
out="${1:-_site}"

rm -rf "$out"
cp -a site "$out"
mkdir -p "$out/assets"
cp -f docs/pulse-mail-logo.png "$out/assets/logo.png"
cp -f docs/assets/demo.gif docs/assets/inbox.png docs/assets/thread.png docs/assets/otp.png "$out/assets/"
rm -f "$out/google-site-verification.content"

token=""
if [[ -f site/google-site-verification.content ]]; then
  token="$(grep -vE '^\s*(#|$)' site/google-site-verification.content | tr -d '[:space:]' || true)"
fi

html_files=("$out/index.html" "$out/privacy/index.html" "$out/terms/index.html")
if [[ -n "$token" ]]; then
  python3 - "$token" "${html_files[@]}" <<'PY'
import sys
from pathlib import Path
token = sys.argv[1]
for path in sys.argv[2:]:
    p = Path(path)
    p.write_text(p.read_text().replace("GOOGLE_SITE_VERIFICATION_TOKEN", token), encoding="utf-8")
PY
  echo "assemble-site: injected Search Console token"
else
  python3 - "${html_files[@]}" <<'PY'
import re, sys
from pathlib import Path
for path in sys.argv[1:]:
    p = Path(path)
    text = p.read_text(encoding="utf-8")
    text = re.sub(r"\s*<!-- Search Console:.*?-->\s*", "\n  ", text, count=1, flags=re.S)
    text = re.sub(r"\s*<meta name=\"google-site-verification\"[^>]*>\s*", "\n  ", text)
    p.write_text(text, encoding="utf-8")
PY
  echo "assemble-site: no Search Console token (meta tag omitted)"
fi
echo "assemble-site: wrote $out"
