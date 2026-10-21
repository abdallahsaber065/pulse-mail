// Builds the srcdoc for the sandboxed email iframe.
//
// Defense in depth:
//  1. <iframe sandbox="allow-same-origin"> : no scripts, forms, popups or
//     top navigation (same-origin only so the parent can size the frame and
//     intercept link clicks; without allow-scripts nothing runs inside).
//  2. A strict CSP meta inside the document: no scripts, no remote loads.
//  3. Remote images are blocked by CSP until the user allows them; then only
//     https images are allowed for that message.

export interface FrameOptions {
  dark: boolean;
  allowRemoteImages: boolean;
}

export function emailSrcdoc(html: string, opts: FrameOptions): string {
  const img = opts.allowRemoteImages ? "data: cid: https:" : "data: cid:";
  const csp = [
    "default-src 'none'",
    `img-src ${img}`,
    "style-src 'unsafe-inline'",
    "font-src data:",
    "script-src 'none'",
    "form-action 'none'",
    "base-uri 'none'",
  ].join("; ");
  const cleaned = stripActiveContent(html);
  return `<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="${csp}">
<meta name="referrer" content="no-referrer">
<style>
  html,body{margin:0;padding:0;background:transparent}
  body{font:14px/1.5 "Segoe UI",system-ui,sans-serif;color:${opts.dark ? "#e3e3e3" : "#1f1f1f"};word-wrap:break-word;overflow-wrap:anywhere}
  img{max-width:100%;height:auto}
  a{color:${opts.dark ? "#a8c7fa" : "#0b57d0"}}
  pre{white-space:pre-wrap}
  table{max-width:100%}
</style></head><body>${cleaned}</body></html>`;
}

/** Plain-text bodies rendered with links preserved. */
export function textSrcdoc(text: string, opts: FrameOptions): string {
  const escaped = escapeHtml(text).replace(
    /\bhttps?:\/\/[^\s<>"']+/g,
    (url) => `<a href="${url}">${url}</a>`,
  );
  return emailSrcdoc(`<pre style="font:inherit;margin:0">${escaped}</pre>`, opts);
}

export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

/** Belt and braces: CSP already blocks these, but don't even ship them. */
export function stripActiveContent(html: string): string {
  return html
    .replace(/<script\b[\s\S]*?<\/script\s*>/gi, "")
    .replace(/<\/?(iframe|object|embed|frame|frameset|applet|base|meta|link|form)\b[^>]*>/gi, "")
    .replace(/\son[a-z]+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, "")
    .replace(/(href|src)\s*=\s*(["']?)\s*javascript:[^"'>\s]*/gi, '$1=$2#');
}
