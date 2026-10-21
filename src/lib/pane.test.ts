import { describe, expect, it } from "vitest";
import { effectivePane, nextPane } from "./pane";

describe("reading pane", () => {
  it("falls back to full page when a split doesn't fit", () => {
    expect(effectivePane("right", 1400, 900)).toBe("right");
    expect(effectivePane("right", 900, 900)).toBe("full");
    expect(effectivePane("bottom", 900, 900)).toBe("bottom");
    expect(effectivePane("bottom", 1400, 500)).toBe("full");
    expect(effectivePane("full", 1400, 900)).toBe("full");
  });
  it("cycles right → bottom → full", () => {
    expect(nextPane("right")).toBe("bottom");
    expect(nextPane("bottom")).toBe("full");
    expect(nextPane("full")).toBe("right");
  });
});
