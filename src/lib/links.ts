// Phishing hint for links in mail: compare the domain a link *shows* with the
// domain it *opens*.

/** Very small "registrable domain" approximation (handles co.uk, com.eg...). */
export function baseDomain(host: string): string {
  const parts = host.toLowerCase().replace(/\.$/, "").split(".");
  if (parts.length <= 2) return parts.join(".");
  const sld = parts[parts.length - 2];
  const short = sld.length <= 3 && ["co", "com", "net", "org", "gov", "edu", "ac", "or", "ne", "go"].includes(sld);
  return parts.slice(short ? -3 : -2).join(".");
}

const DOMAIN_IN_TEXT = /(?:https?:\/\/)?((?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,24})(?=[/:?#\s]|$)/i;

export interface LinkCheck {
  /** Host the link really opens. */
  host: string;
  /** Domain shown in the link text, if any. */
  shown: string | null;
  /** Shown and real domains differ. */
  mismatch: boolean;
  /** Host uses punycode (possible look-alike characters). */
  punycode: boolean;
  /** Plain http (not https). */
  insecure: boolean;
}

export function checkLink(href: string, text: string): LinkCheck | null {
  let url: URL;
  try {
    url = new URL(href);
  } catch {
    return null;
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") return null;
  const host = url.hostname.toLowerCase();
  const m = text.trim().match(DOMAIN_IN_TEXT);
  const shown = m ? m[1].toLowerCase() : null;
  return {
    host,
    shown,
    mismatch: !!shown && baseDomain(shown) !== baseDomain(host),
    punycode: host.split(".").some((l) => l.startsWith("xn--")),
    insecure: url.protocol === "http:",
  };
}
