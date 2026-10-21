// Compose helpers (pure): drafts from reply context, quote headers, mailto.
import type { ComposeInit, ComposeMode, Draft, DraftAttachment } from "./types";

/** Gmail's limit for the files of one message. */
export const MAX_ATTACH_TOTAL = 25 * 1024 * 1024;

export const attachTotal = (list: DraftAttachment[]) => list.reduce((n, a) => n + a.size, 0);

export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

/** Plain text → editor paragraphs (blank lines split paragraphs). */
export function textToHtml(text: string): string {
  if (!text) return "";
  return text
    .replace(/\r\n/g, "\n")
    .split(/\n{2,}/)
    .map((p) => `<p>${escapeHtml(p).replace(/\n/g, "<br>")}</p>`)
    .join("");
}

/** Add the account signature below the cursor position (Gmail style "-- "). */
export function withSignature(d: Draft, signature: string): Draft {
  const sig = signature.trim();
  if (!sig) return d;
  const html = (d.html ?? textToHtml(d.body)) + `<p></p><p>-- <br>${escapeHtml(sig).replace(/\n/g, "<br>")}</p>`;
  return { ...d, html, body: `${d.body}\n\n-- \n${sig}` };
}

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
  html: null,
  attachments: [],
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
    d.attachments = init.attachments ?? [];
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
export const hasContent = (d: Draft) =>
  !!(d.to.trim() || d.cc.trim() || d.bcc.trim() || d.subject.trim() || d.body.trim() || d.attachments.length);

const attachKey = (l: DraftAttachment[]) => l.map((a) => a.staged ?? `${a.sourceMessage}/${a.sourcePart}`).join("|");

/** Same content (ignores ids), to skip redundant autosaves. */
export const sameContent = (a: Draft, b: Draft) =>
  a.to === b.to && a.cc === b.cc && a.bcc === b.bcc && a.subject === b.subject && a.body === b.body &&
  (a.html ?? null) === (b.html ?? null) &&
  attachKey(a.attachments) === attachKey(b.attachments) &&
  (a.quote?.text ?? null) === (b.quote?.text ?? null);

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
