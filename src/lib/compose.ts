// Compose helpers (pure): drafts from reply context, quote headers, mailto.
import type { ComposeInit, ComposeMode, Draft } from "./types";

export const emptyDraft = (): Draft => ({
  to: "",
  cc: "",
  bcc: "",
  subject: "",
  body: "",
  quote: null,
  inReplyTo: "",
  references: "",
  threadId: null,
  draftId: null,
});

/** "Tue, Oct 21, 2026 at 9:34 PM" (Gmail's quote date style, English UI). */
export function quoteDate(ms: number): string {
  const d = new Date(ms);
  const day = d.toLocaleDateString("en-US", { weekday: "short", month: "short", day: "numeric", year: "numeric" });
  const time = d.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" });
  return `${day} at ${time}`;
}

const plain = (addr: string) => addr.replace(/"/g, "");

export function draftFrom(init: ComposeInit, mode: Exclude<ComposeMode, "new">): Draft {
  const d = emptyDraft();
  d.to = init.to;
  d.cc = init.cc;
  d.subject = init.subject;
  d.inReplyTo = init.inReplyTo;
  d.references = init.references;
  d.threadId = init.threadId;
  if (mode === "forward") {
    const lines = [
      "---------- Forwarded message ---------",
      `From: ${plain(init.originalFrom)}`,
      `Date: ${quoteDate(init.originalDate)}`,
      `Subject: ${init.originalSubject}`,
      `To: ${init.originalTo}`,
    ];
    if (init.originalCc) lines.push(`Cc: ${init.originalCc}`);
    d.quote = { header: lines.join("\n"), text: init.originalText, forward: true };
  } else {
    d.quote = { header: `On ${quoteDate(init.originalDate)} ${plain(init.originalFrom)} wrote:`, text: init.originalText, forward: false };
  }
  return d;
}

/** Anything worth saving as a draft? */
export const hasContent = (d: Draft) => !!(d.to.trim() || d.cc.trim() || d.bcc.trim() || d.subject.trim() || d.body.trim());

/** Same content (ignores ids), to skip redundant autosaves. */
export const sameContent = (a: Draft, b: Draft) =>
  a.to === b.to && a.cc === b.cc && a.bcc === b.bcc && a.subject === b.subject && a.body === b.body && (a.quote?.text ?? null) === (b.quote?.text ?? null);

/** A `mailto:` link → draft fields (to, cc, bcc, subject, body). */
export function parseMailto(href: string): Draft | null {
  if (!/^mailto:/i.test(href)) return null;
  const d = emptyDraft();
  const rest = href.slice(7);
  const [addr, query = ""] = rest.split("?", 2);
  const dec = (s: string) => {
    try {
      return decodeURIComponent(s.replace(/\+/g, "%20"));
    } catch {
      return s;
    }
  };
  d.to = dec(addr);
  for (const pair of query.split("&")) {
    const [k, v = ""] = pair.split("=", 2);
    const key = k.toLowerCase();
    const val = dec(v);
    if (key === "cc") d.cc = val;
    else if (key === "bcc") d.bcc = val;
    else if (key === "subject") d.subject = val.slice(0, 500);
    else if (key === "body") d.body = val.slice(0, 100_000);
    else if (key === "to") d.to = [d.to, val].filter(Boolean).join(", ");
  }
  return d;
}
