import { describe, expect, it } from "vitest";
import { emptySelection, selectAll, toggleSelected, isSelected, selectedCount, threadKey } from "./selection";

describe("conversation selection", () => {
  it("keeps select all constant sized for a million conversations", () => {
    const all = selectAll({ id: 7, total: 1_000_000 });
    expect(all.exceptions.size).toBe(0);
    expect(selectedCount(all)).toBe(1_000_000);
    const except = toggleSelected(all, "1:abc");
    expect(selectedCount(except)).toBe(999_999);
    expect(isSelected(except, "1:abc", true)).toBe(false);
    expect(isSelected(except, "2:abc", true)).toBe(true);
    expect(isSelected(except, "1:new-arrival", false)).toBe(false);
    expect(selectedCount(toggleSelected(except, "1:abc"))).toBe(1_000_000);
  });
  it("keeps identical Gmail ids in different accounts independent", () => {
    const key = threadKey({ accountId: 1, threadId: "abc" });
    const one = toggleSelected(emptySelection(), key);
    expect(selectedCount(one)).toBe(1);
    expect(isSelected(one, "1:abc", false)).toBe(true);
    expect(isSelected(one, "2:abc", false)).toBe(false);
    expect(selectedCount(toggleSelected(one, key))).toBe(0);
  });
});
