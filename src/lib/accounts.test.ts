import { describe, expect, it } from "vitest";
import { combineSync, compactCount, formatEta, historyBar, historyFraction, historyLine, phaseLabel, sizeLine } from "./accounts";
import type { History, SyncStatus } from "./types";

const s = (over: Partial<SyncStatus>): SyncStatus => ({
  accountId: 1,
  email: "a@x.com",
  running: true,
  imapConnected: true,
  syncing: false,
  foreground: false,
  lastSyncAt: 1000,
  error: null,
  progress: null,
  backfill: null,
  history: null,
  ...over,
});
const h = (over: Partial<History>): History => ({ state: "running", window: "1y", phase: "index", done: 0, total: 0, rate: 0, etaSecs: null, error: null, bytesTotal: 0, bytesText: 0, ...over });

describe("combineSync", () => {
  it("passes a single account through", () => {
    const one = s({});
    expect(combineSync([one])).toBe(one);
  });
  it("is live only if every account is live, with the stalest age", () => {
    const c = combineSync([s({ lastSyncAt: 5000 }), s({ accountId: 2, email: "b@x.com", lastSyncAt: 2000, imapConnected: false })])!;
    expect(c.imapConnected).toBe(false);
    expect(c.lastSyncAt).toBe(2000);
  });
  it("names the account with an error", () => {
    const c = combineSync([s({}), s({ accountId: 2, email: "b@x.com", error: "Please sign in again." })])!;
    expect(c.error).toBe("b@x.com: Please sign in again.");
  });
  it("background checks stay quiet", () => {
    const c = combineSync([s({ syncing: true }), s({ accountId: 2 })])!;
    expect(c.syncing).toBe(false);
  });
});

describe("history helpers", () => {
  it("fraction", () => {
    expect(historyFraction(h({ done: 25, total: 100 }))).toBe(0.25);
    expect(historyFraction(h({ total: 0 }))).toBeNull();
    expect(historyFraction(h({ state: "done" }))).toBe(1);
  });
  it("eta", () => {
    expect(formatEta(null)).toMatch(/Estimating/);
    expect(formatEta(30)).toBe("Less than a minute left");
    expect(formatEta(300)).toBe("About 5 min left");
    expect(formatEta(7800)).toBe("About 2 h 10 min left");
  });
  it("phase labels", () => {
    expect(phaseLabel(h({ phase: "bodies" }))).toBe("Downloading messages");
    expect(phaseLabel(h({ phase: "count" }))).toBe("Counting messages…");
    expect(phaseLabel(h({ state: "paused" }))).toBe("Paused");
    expect(phaseLabel(h({ error: "Gmail stopped responding." }))).toBe("Download paused");
  });
  it("progress line, bar and sizes", () => {
    expect(historyLine(h({ phase: "count", done: 12340, total: 12340 }))).toBe("12,340 found");
    expect(historyBar(h({ phase: "count", done: 500, total: 500 }))).toBeNull();
    expect(historyLine(h({ phase: "index", done: 1234, total: 12340, rate: 45.2, etaSecs: 240 }))).toBe("1,234 / 12,340 · 45/s · ~4 min left");
    expect(historyLine(h({ phase: "index", done: 10, total: 20, state: "paused" }))).toBe("10 / 20");
    const fmt = (b: number) => `${Math.round(b / 1e6)} MB`;
    expect(sizeLine(h({ bytesTotal: 2_000_000_000, bytesText: 400_000_000 }), fmt)).toBe("≈ 2000 MB in Gmail, 400 MB without attachments");
    expect(sizeLine(h({}), fmt)).toBe("");
  });
  it("compact counts", () => {
    expect(compactCount(950)).toBe("950");
    expect(compactCount(1240)).toBe("1.2k");
    expect(compactCount(48_312)).toBe("48k");
    expect(compactCount(1_200_000)).toBe("1.2M");
  });
});
