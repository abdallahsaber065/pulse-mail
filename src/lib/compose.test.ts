import { describe, expect, it } from "vitest";
import { draftFrom, emptyDraft, hasContent, parseMailto, sameContent } from "./compose";
import type { ComposeInit } from "./types";

const init: ComposeInit = {
  to: '"Ana Lima" <ana@x.test>',
  cc: "carol@z.test",
  subject: "Re: Q4",
  inReplyTo: "<abc@x.test>",
  references: "<abc@x.test>",
  threadId: "t1",
  originalFrom: '"Ana Lima" <ana@x.test>',
  originalTo: "me@gmail.com",
  originalCc: "",
  originalDate: Date.UTC(2026, 9, 21, 18, 34),
  originalSubject: "Q4",
  originalText: "numbers attached",
};

describe("compose helpers", () => {
  it("builds reply and forward drafts", () => {
    const r = draftFrom(init, "reply");
    expect(r.to).toBe('"Ana Lima" <ana@x.test>');
    expect(r.threadId).toBe("t1");
    expect(r.quote?.header).toMatch(/^On .*2026 at .* Ana Lima <ana@x.test> wrote:$/);
    expect(r.quote?.forward).toBe(false);
    const f = draftFrom({ ...init, to: "", cc: "", threadId: null, subject: "Fwd: Q4" }, "forward");
    expect(f.quote?.header.split("\n")[0]).toBe("---------- Forwarded message ---------");
    expect(f.quote?.header).toContain("Subject: Q4");
    expect(f.quote?.forward).toBe(true);
  });
  it("parses mailto links", () => {
    const d = parseMailto("mailto:a@x.test?cc=b@y.test&subject=Hello%20there&body=Line%201%0ALine%202")!;
    expect(d.to).toBe("a@x.test");
    expect(d.cc).toBe("b@y.test");
    expect(d.subject).toBe("Hello there");
    expect(d.body).toBe("Line 1\nLine 2");
    expect(parseMailto("https://x.test")).toBeNull();
    expect(parseMailto("mailto:a@x.test?subject=%E0%A4%A")!.subject).toBe("%E0%A4%A");
  });
  it("detects content and changes", () => {
    const a = emptyDraft();
    expect(hasContent(a)).toBe(false);
    const b = { ...a, body: "hi" };
    expect(hasContent(b)).toBe(true);
    expect(sameContent(a, b)).toBe(false);
    expect(sameContent(b, { ...b, draftId: "d1" })).toBe(true);
  });
});
