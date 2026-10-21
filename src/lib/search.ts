// Merging local (instant, cached) and Gmail server search results.
import type { ThreadRow } from "./types";

const key = (t: ThreadRow) => `${t.accountId}:${t.threadId}`;

/**
 * Local rows first win (they reflect local changes), server rows add what
 * isn't cached; newest first.
 */
export function mergeResults(local: ThreadRow[], server: ThreadRow[]): ThreadRow[] {
  const seen = new Map<string, ThreadRow>();
  for (const t of local) seen.set(key(t), t);
  for (const t of server) if (!seen.has(key(t))) seen.set(key(t), t);
  return [...seen.values()].sort((a, b) => b.internalDate - a.internalDate);
}

/** Gmail operators the local index can't answer (in:, label:, before:, -x, OR …). */
export function needsServer(q: string): boolean {
  const local = new Set(["from", "subject", "is", "has", "http", "https", "re", "fwd", "fw"]);
  return q
    .split(/\s+/)
    .some((t) => t === "OR" || (t.length > 1 && t.startsWith("-")) || ((m) => !!m && !local.has(m[1].toLowerCase()))(/^([a-z_]+):/i.exec(t)));
}

/** Server search is worth it for 2+ characters of query. */
export const serverSearchable = (q: string) => q.trim().length >= 2;
