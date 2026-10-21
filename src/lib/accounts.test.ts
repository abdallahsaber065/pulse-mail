import { describe, expect, it } from "vitest";
import { combineSync, compactCount, formatEta, historyFraction, phaseLabel } from "./accounts";
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
const h = (over: Partial<History>): History => ({ state: "running", window: "1y", phase: "list", done: 0, total: 0, rate: 0, etaSecs: null, ...over });

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
    expect(phaseLabel(h({ phase: "bodies" }))).toBe("Saving offline copies");
    expect(phaseLabel(h({ state: "paused" }))).toBe("Paused");
  });
  it("compact counts", () => {
    expect(compactCount(950)).toBe("950");
    expect(compactCount(1240)).toBe("1.2k");
    expect(compactCount(48_312)).toBe("48k");
    expect(compactCount(1_200_000)).toBe("1.2M");
  });
});
