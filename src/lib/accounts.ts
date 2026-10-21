// Pure helpers for multi-account status and the history download (H1).
import type { History, SyncStatus, SyncWindow } from "./types";

export const WINDOWS: { id: SyncWindow; label: string; short: string }[] = [
  { id: "30d", label: "Last 30 days", short: "30 days" },
  { id: "90d", label: "Last 90 days", short: "90 days" },
  { id: "1y", label: "Last year", short: "1 year" },
  { id: "all", label: "Everything", short: "Everything" },
];

export const windowLabel = (w: SyncWindow) => WINDOWS.find((x) => x.id === w)?.label ?? w;

/**
 * One status for the chip and the top bar from every account's status.
 * Errors and foreground work win; "Live" only when every running account is
 * live; the age is the stalest account's.
 */
export function combineSync(list: SyncStatus[]): SyncStatus | null {
  if (list.length === 0) return null;
  if (list.length === 1) return list[0];
  const running = list.filter((s) => s.running);
  const multi = list.length > 1;
  const err = list.find((s) => s.error);
  const fg = list.find((s) => s.progress);
  const ages = running.map((s) => s.lastSyncAt).filter((x): x is number => x != null);
  return {
    accountId: 0,
    email: "",
    running: running.length > 0,
    imapConnected: running.length > 0 && running.every((s) => s.imapConnected),
    syncing: list.some((s) => s.syncing && s.foreground),
    foreground: list.some((s) => s.syncing && s.foreground),
    lastSyncAt: ages.length ? Math.min(...ages) : null,
    error: err ? (multi ? `${err.email}: ${err.error}` : err.error) : null,
    progress: fg?.progress ?? null,
    backfill: list.find((s) => s.backfill)?.backfill ?? null,
    history: null,
  };
}

/** 0..1, or null when the total isn't known yet. */
export function historyFraction(h: History | null | undefined): number | null {
  if (!h) return null;
  if (h.state === "done") return 1;
  return h.total > 0 ? Math.min(1, h.done / h.total) : null;
}

/** "about 5 min left", "about 2 h 10 min left", "less than a minute left". */
export function formatEta(secs: number | null | undefined): string {
  if (secs == null) return "Estimating time left…";
  if (secs < 60) return "Less than a minute left";
  const m = Math.round(secs / 60);
  if (m < 60) return `About ${m} min left`;
  const h = Math.floor(m / 60);
  const r = m % 60;
  return r ? `About ${h} h ${r} min left` : `About ${h} h left`;
}

export const formatRate = (r: number) => (r >= 1 ? `${Math.round(r)} messages/s` : r > 0 ? `${r.toFixed(1)} messages/s` : "");

/** Phase label for the progress screen. */
export function phaseLabel(h: History): string {
  if (h.state === "done") return "All set";
  if (h.state === "setup") return "Waiting for your choices";
  if (h.error) return "Download paused";
  if (h.state === "paused") return "Paused";
  switch (h.phase) {
    case "count":
      return "Counting messages…";
    case "index":
      return "Indexing messages";
    case "bodies":
      return "Downloading messages";
    case "files":
      return "Downloading attachments…";
  }
  return h.state === "running" ? "Getting ready…" : "Waiting to start";
}

const n = (x: number) => x.toLocaleString("en-US");

/**
 * One-line progress: "12,340 found", "1,234 / 12,340 · 45/s · About 4 min left".
 * The count phase has no known end, so it shows only what was found.
 */
export function historyLine(h: History): string {
  if (h.state === "done") return "";
  if (h.phase === "count") return `${n(h.done)} found`;
  const parts = [h.total > 0 ? `${n(h.done)} / ${n(h.total)}` : n(h.done)];
  if (h.state === "running" && !h.error) {
    if (h.rate > 0) parts.push(`${h.rate >= 10 ? Math.round(h.rate) : h.rate.toFixed(1)}/s`);
    parts.push(formatEta(h.etaSecs).replace("About ", "~"));
  }
  return parts.join(" · ");
}

/** Progress bar fraction; null while counting (indeterminate). */
export function historyBar(h: History | null | undefined): number | null {
  if (!h) return null;
  if (h.state === "done") return 1;
  if (h.phase === "count" || h.phase === "") return null;
  return historyFraction(h);
}

/** "≈ 2.3 GB, 410 MB without attachments" once sizes are known. */
export function sizeLine(h: History | null | undefined, fmt: (b: number) => string): string {
  if (!h || h.bytesTotal <= 0) return "";
  const text = h.bytesText > 0 && h.bytesText < h.bytesTotal * 0.95 ? `, ${fmt(h.bytesText)} without attachments` : "";
  return `≈ ${fmt(h.bytesTotal)} in Gmail${text}`;
}

/** Compact counts: 950, 12.4k, 1.2M. */
export function compactCount(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(n < 10_000 ? 1 : 0)}k`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}
