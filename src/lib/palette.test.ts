import { describe, expect, it } from "vitest";
import { rank, score, type Command } from "./palette";

const cmd = (title: string, group = "Actions", words = ""): Command => ({ id: title, group, title, words, run() {} });

describe("palette", () => {
  it("scores prefixes above substrings above subsequences", () => {
    expect(score("arch", "Archive")).toBeGreaterThan(score("arch", "Unarchive"));
    expect(score("read", "Mark as read")).toBeGreaterThan(0);
    expect(score("mrk rd", "Mark as read")).toBeGreaterThan(0);
    expect(score("zzz", "Archive")).toBe(0);
  });
  it("ranks and matches synonyms through words", () => {
    const list = [cmd("Go to Inbox", "Go to"), cmd("Archive"), cmd("Reading pane: full page", "Layout", "no split")];
    expect(rank("inb", list)[0].title).toBe("Go to Inbox");
    expect(rank("split", list)[0].title).toBe("Reading pane: full page");
    expect(rank("", list)).toHaveLength(3);
  });
});
