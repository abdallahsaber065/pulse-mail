import { describe, expect, it } from "vitest";
import { avatarHue, initials, labelName, listDate, relative } from "./format";

describe("format", () => {
  it("names system labels like Gmail", () => {
    expect(labelName("INBOX", "INBOX")).toBe("Inbox");
    expect(labelName("DRAFT", "DRAFT")).toBe("Drafts");
    expect(labelName("Label_7", "Clients")).toBe("Clients");
  });

  it("shows time for today and a date otherwise", () => {
    const now = new Date(2026, 9, 21, 15, 0).getTime();
    expect(listDate(new Date(2026, 9, 21, 9, 5).getTime(), now)).toMatch(/9:05/);
    expect(listDate(new Date(2026, 2, 3).getTime(), now)).toMatch(/3/);
    expect(listDate(new Date(2024, 2, 3).getTime(), now)).toMatch(/24/);
  });

  it("relative times", () => {
    const now = 1_000_000_000;
    expect(relative(now - 10_000, now)).toBe("just now");
    expect(relative(now - 60_000, now)).toBe("1 minute ago");
    expect(relative(now - 3 * 3_600_000, now)).toBe("3 hours ago");
  });

  it("initials", () => {
    expect(initials("Sara Ahmed")).toBe("SA");
    expect(initials("", "noreply@github.com")).toBe("N");
    expect(initials("GitHub")).toBe("G");
    expect(initials("abdallahsaber00002@gmail.com", "abdallahsaber00002@gmail.com")).toBe("A");
    expect(initials("Abdallah Saber", "abdallahsaber00002@gmail.com")).toBe("AS");
    expect(initials("Abdallah Mohamed Saber")).toBe("AS");
    expect(initials("")).toBe("?");
  });

  it("stable avatar hue", () => {
    expect(avatarHue("a@b.c")).toBe(avatarHue("a@b.c"));
    expect(avatarHue("x")).toBeGreaterThanOrEqual(0);
    expect(avatarHue("x")).toBeLessThan(360);
  });
});
