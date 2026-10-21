import { describe, expect, it } from "vitest";
import { emailSrcdoc, stripActiveContent, textSrcdoc } from "./email-frame";

describe("email frame", () => {
  it("strips scripts, handlers and javascript: urls", () => {
    const out = stripActiveContent(
      `<p onclick="x()">hi</p><script>alert(1)</script><a href="javascript:alert(1)">x</a><iframe src="https://e.vil"></iframe><meta http-equiv="refresh" content="0;url=https://e.vil">`,
    );
    expect(out).not.toMatch(/script|onclick|javascript:|iframe|refresh/i);
    expect(out).toContain("<p>hi</p>");
  });

  it("blocks remote images unless allowed", () => {
    expect(emailSrcdoc("<img src=https://t.example/p.gif>", { dark: false, allowRemoteImages: false })).toContain("img-src data: cid:;");
    expect(emailSrcdoc("x", { dark: false, allowRemoteImages: true })).toContain("img-src data: cid: https:");
  });

  it("escapes plain text and linkifies urls", () => {
    const out = textSrcdoc("<b>hi</b> see https://example.com/a?b=1", { dark: true, allowRemoteImages: false });
    expect(out).toContain("&lt;b&gt;hi&lt;/b&gt;");
    expect(out).toContain('<a href="https://example.com/a?b=1">');
  });
});
