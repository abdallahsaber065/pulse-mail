import { describe, expect, it } from "vitest";
import { mergeResults, needsServer, serverSearchable } from "./search";
import type { ThreadRow } from "./types";
import { toggleSearchFilter, hasSearchFilter } from "./search";

const row = (accountId: number, threadId: string, internalDate: number, subject = threadId): ThreadRow =>
  ({ accountId, threadId, lastMessageId: threadId, subject, fromName: "", fromEmail: "", snippet: "", internalDate, messageCount: 1, unread: false, starred: false, otpCode: null }) as ThreadRow;

describe("search merge", () => {
  it("dedupes by account + thread, local wins, newest first", () => {
    const out = mergeResults([row(1, "a", 10, "local")], [row(1, "a", 10, "server"), row(2, "a", 30), row(1, "old", 1)]);
    expect(out.map((t) => `${t.accountId}:${t.threadId}`)).toEqual(["2:a", "1:a", "1:old"]);
    expect(out[1].subject).toBe("local");
  });
  it("knows which queries need Gmail", () => {
    expect(needsServer("from:ana invoice")).toBe(false);
    expect(needsServer("in:sent hello")).toBe(true);
    expect(needsServer("before:2024/01/01")).toBe(true);
    expect(needsServer("-promo")).toBe(true);
    expect(needsServer("https://x.test")).toBe(false);
    expect(needsServer('subject:"OR report"')).toBe(false);
    expect(needsServer("is:important")).toBe(true);
    expect(needsServer("has:drive")).toBe(true);
    expect(serverSearchable("a")).toBe(false);
    expect(serverSearchable("ab")).toBe(true);
  });
  it("toggles filters without damaging quoted phrases, Arabic or conflicting read filters", () => {
    expect(toggleSearchFilter('subject:"is:unread report" فاتورة', "is:unread")).toBe('subject:"is:unread report" فاتورة is:unread');
    expect(toggleSearchFilter('is:read from:"سارة أحمد"', "is:unread")).toBe('from:"سارة أحمد" is:unread');
    expect(toggleSearchFilter('hello IS:UNREAD', "is:unread")).toBe("hello");
    expect(hasSearchFilter('"is:unread" hello', "is:unread")).toBe(false);
    expect(hasSearchFilter('hello HAS:ATTACHMENT', "has:attachment")).toBe(true);
  });
});
