import { describe, expect, it } from "vitest";
import { applySuggestion, getActiveToken, getSearchSuggestions } from "./search-syntax";

describe("search-syntax", () => {
  it("extracts the active token at the end or cursor", () => {
    expect(getActiveToken("hello world").token).toBe("world");
    expect(getActiveToken("is:").token).toBe("is:");
    expect(getActiveToken("from:me has:").token).toBe("has:");
    expect(getActiveToken("from:me has: ", 4).token).toBe("from:me");
    expect(getActiveToken("").token).toBe("");
  });

  it("suggests syntax based on active prefix", () => {
    const isSuggestions = getSearchSuggestions("is:");
    expect(isSuggestions.some((s) => s.token === "is:unread")).toBe(true);
    expect(isSuggestions.some((s) => s.token === "is:starred")).toBe(true);

    const hasSuggestions = getSearchSuggestions("has:");
    expect(hasSuggestions.some((s) => s.token === "has:attachment")).toBe(true);

    const catSuggestions = getSearchSuggestions("cat");
    expect(catSuggestions.some((s) => s.token === "category:primary")).toBe(true);
    expect(catSuggestions.some((s) => s.token === "category:promotions")).toBe(true);

    const emptySuggestions = getSearchSuggestions("");
    expect(emptySuggestions.length).toBeGreaterThan(0);
  });

  it("applies a suggestion replacing the active token", () => {
    const { newQuery } = applySuggestion("hello is:", "is:unread");
    expect(newQuery).toBe("hello is:unread ");

    const { newQuery: fromQuery } = applySuggestion("from", "from:");
    expect(fromQuery).toBe("from:");

    const { newQuery: midQuery } = applySuggestion("subject:test cat after:2026", "category:promotions", 15);
    expect(midQuery).toContain("category:promotions");
  });
});

