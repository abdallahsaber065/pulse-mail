// Pure mapping from the backend SyncStatus to what the status chip shows.
// Kept separate from the component so it is unit-tested.
import { relative } from "./format";
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
  if (s.syncing) return { text: "Checking for mail…", tone: "busy", bar: null, retry: false };
  if (s.backfill) {
    const { done, total } = s.backfill;
    return {
      text: total > 0 ? `Syncing ${done}/${total}` : "Syncing older mail…",
      tone: "busy",
      bar: pct(done, total),
      barSubtle: true,
      retry: false,
    };
  }
  const when = s.lastSyncAt ? relative(s.lastSyncAt, now) : null;
  if (s.imapConnected) return { text: when ? `Live · ${when}` : "Live", tone: "live", retry: false };
  return { text: when ? `Up to date · ${when}` : "Up to date", tone: "ok", retry: false };
}
