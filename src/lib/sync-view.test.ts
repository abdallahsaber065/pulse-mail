import { describe, expect, it } from "vitest";
import { chipView } from "./sync-view";
import type { SyncStatus } from "./types";

const base: SyncStatus = {
  accountId: 1,
  email: "a@x.com",
  history: null,
  running: true,
  imapConnected: false,
  syncing: false,
  foreground: false,
  lastSyncAt: null,
  error: null,
  progress: null,
  backfill: null,
};
const now = 1_000_000_000;

describe("chipView", () => {
  it("shows offline first", () => {
    expect(chipView({ ...base, error: "x" }, false, now).tone).toBe("warn");
  });
  it("error offers retry", () => {
    const v = chipView({ ...base, error: "Couldn't reach Gmail." }, true, now);
    expect(v).toMatchObject({ tone: "error", retry: true, text: "Couldn't reach Gmail." });
  });
  it("foreground progress is determinate", () => {
    const v = chipView({ ...base, syncing: true, progress: { label: "Getting your newest mail", done: 20, total: 50 } }, true, now);
    expect(v.text).toBe("Getting your newest mail 20/50");
    expect(v.bar).toBeCloseTo(0.4);
  });
  it("a manual check is indeterminate", () => {
    expect(chipView({ ...base, syncing: true, foreground: true }, true, now).bar).toBeNull();
  });
  it("background checks are quiet: no bar, chip stays Live", () => {
    const v = chipView({ ...base, imapConnected: true, syncing: true, lastSyncAt: now - 2 * 60_000 }, true, now);
    expect(v).toMatchObject({ text: "Live · 2m", tone: "live" });
    expect(v.bar).toBeUndefined();
  });
  it("history download keeps the chip and draws a subtle bar", () => {
    const v = chipView({ ...base, imapConnected: true, lastSyncAt: now, backfill: { label: "Older mail", done: 120, total: 300 } }, true, now);
    expect(v).toMatchObject({ text: "Live · now", barSubtle: true });
    expect(v.bar).toBeCloseTo(0.4);
  });
  it("idle states", () => {
    expect(chipView({ ...base, imapConnected: true, lastSyncAt: now - 5_000 }, true, now).text).toBe("Live · now");
    expect(chipView({ ...base, lastSyncAt: now - 3 * 60_000 }, true, now).text).toBe("Up to date · 3m");
    expect(chipView({ ...base, running: false }, true, now).retry).toBe(true);
  });
});
