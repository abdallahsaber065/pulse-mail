// Pure mapping from the backend SyncStatus to what the status chip shows.
// Kept separate from the component so it is unit-tested.
import type { SyncStatus } from "./types";

export type ChipTone = "live" | "ok" | "busy" | "warn" | "error" | "idle";

export interface ChipView {
  text: string;
  tone: ChipTone;
  /** 0..1 determinate, null = indeterminate, undefined = no bar. */
  bar?: number | null;
  /** Bar is for background work (drawn subtler). */
  barSubtle?: boolean;
  retry: boolean;
}

/** Compact age for the chip: "now", "2m", "3h", "4d". */
export function shortAgo(at: number, now: number): string {
  const s = Math.max(0, Math.round((now - at) / 1000));
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86_400) return `${Math.floor(s / 3600)}h`;
  return `${Math.floor(s / 86_400)}d`;
}

const pct = (done: number, total: number) => (total > 0 ? Math.min(1, done / total) : null);

export function chipView(s: SyncStatus | null, online: boolean, now = Date.now()): ChipView {
  if (!online) return { text: "Offline · showing saved mail", tone: "warn", retry: false };
  if (!s || !s.running) return { text: "Not syncing", tone: "idle", retry: true };
  if (s.error) return { text: s.error, tone: "error", retry: true };
  if (s.progress) {
    const { label, done, total } = s.progress;
    return {
      text: total > 0 ? `${label} ${done}/${total}` : `${label}…`,
      tone: "busy",
      bar: pct(done, total),
      retry: false,
    };
  }
  // Background checks (push, poll, retries) never change the chip: it stays
  // "Live · now". Only user-initiated passes show "Checking for mail…".
  if (s.syncing && s.foreground) return { text: "Checking for mail…", tone: "busy", bar: null, retry: false };
  const when = s.lastSyncAt ? shortAgo(s.lastSyncAt, now) : null;
  const base: ChipView = s.imapConnected
    ? { text: when ? `Live · ${when}` : "Live", tone: "live", retry: false }
    : { text: when ? `Up to date · ${when}` : "Up to date", tone: "ok", retry: false };
  // History download: a subtle bar only; details live in the popover.
  if (s.backfill) return { ...base, bar: pct(s.backfill.done, s.backfill.total), barSubtle: true };
  return base;
}
