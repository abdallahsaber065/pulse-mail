import { describe, expect, it } from "vitest";
import { attachTotal, draftFrom, draftFromMessage, emptyDraft, hasContent, parseMailto, sameContent, textToHtml, withSignature } from "./compose";
import type { ComposeInit, Message } from "./types";

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
  attachments: [],
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

describe("rich compose helpers", () => {
  it("text to editor html, escaped", () => {
    expect(textToHtml("a <b>\nline\n\nnext")).toBe("<p>a &lt;b&gt;<br>line</p><p>next</p>");
    expect(textToHtml("")).toBe("");
  });
  it("signature goes below the body in both parts", () => {
    const d = withSignature({ ...emptyDraft(), body: "Hi" }, "Abdallah\n<CEO>");
    expect(d.body).toBe("Hi\n\n-- \nAbdallah\n<CEO>");
    expect(d.html).toContain("-- <br>Abdallah<br>&lt;CEO&gt;");
    expect(withSignature(emptyDraft(), "  ").html).toBeNull();
  });
  it("attachments count as content and change detection", () => {
    const a = emptyDraft();
    const b = { ...a, attachments: [{ name: "x.pdf", mime: "application/pdf", size: 10, staged: "s1" }] };
    expect(hasContent(b)).toBe(true);
    expect(sameContent(a, b)).toBe(false);
    expect(attachTotal(b.attachments)).toBe(10);
  });
  it("converts existing message into an editable draft", () => {
    const msg: Message = {
      id: "m_draft1",
      threadId: "t_draft1",
      internalDate: 1700000000,
      fromName: "Me",
      fromEmail: "me@example.test",
      toList: '"Partner" <partner@example.test>',
      ccList: "team@example.test",
      replyTo: "",
      rfcMessageId: "<draft1@example.test>",
      rfcReferences: "",
      subject: "Important proposal",
      snippet: "Please find proposal below.",
      bodyText: "Please find proposal below.",
      bodyHtml: "<p>Please find proposal below.</p>",
      bodyLoaded: true,
      labelIds: ["DRAFT"],
      unread: false,
      starred: false,
      hasAttachments: true,
      attachments: [{ part: 1, filename: "proposal.pdf", mime: "application/pdf", size: 1024, inline: false }],
      otpCode: null,
    };
    const draft = draftFromMessage(msg);
    expect(draft.to).toBe('"Partner" <partner@example.test>');
    expect(draft.cc).toBe("team@example.test");
    expect(draft.subject).toBe("Important proposal");
    expect(draft.body).toBe("Please find proposal below.");
    expect(draft.html).toBe("<p>Please find proposal below.</p>");
    expect(draft.draftId).toBe("m_draft1");
    expect(draft.threadId).toBe("t_draft1");
    expect(draft.attachments.length).toBe(1);
    expect(draft.attachments[0].name).toBe("proposal.pdf");
  });
});
