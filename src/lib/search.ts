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
  return searchTokens(q).some((t) => {
    if (t === "OR" || t.startsWith("{") || t.startsWith("(") || (t.length > 1 && t.startsWith("-"))) return true;
    const m = /^([a-z_]+):(.*)$/i.exec(t);
    if (!m) return false;
    const op = m[1].toLowerCase(), value = m[2].toLowerCase();
    if (!local.has(op)) return true;
    if (op === "is") return !["unread", "read", "starred"].includes(value);
    if (op === "has") return value !== "attachment";
    return (op === "from" || op === "subject") && !value.replace(/"/g, "").trim();
  });
}

/** Keep quoted phrases together, including operator values and Arabic text. */
function searchTokens(query: string): string[] {
  const out: string[] = [];
  let token = "", quoted = false;
  for (const char of query) {
    if (char === '"') quoted = !quoted;
    if (/\s/u.test(char) && !quoted) {
      if (token) out.push(token);
      token = "";
    } else token += char;
  }
  if (token) out.push(token);
  return out;
}
export type SearchFilter = "is:unread" | "is:starred" | "has:attachment";
export const hasSearchFilter = (query: string, filter: SearchFilter) => searchTokens(query).some((t) => t.toLowerCase() === filter);
export function toggleSearchFilter(query: string, filter: SearchFilter): string {
  const active = hasSearchFilter(query, filter);
  const tokens = searchTokens(query).filter((t) => t.toLowerCase() !== filter && !(filter === "is:unread" && t.toLowerCase() === "is:read"));
  if (!active) tokens.push(filter);
  return tokens.join(" ");
}

/** Server search is worth it for 2+ characters of query. */
export const serverSearchable = (q: string) => q.trim().length >= 2;

/**
 * Search activates from two characters (like Gmail): a single letter is a
 * prefix over almost every word in the index, so the first keystroke would
 * trigger the heaviest scan for the least information. One non-ASCII
 * character (CJK, Arabic) can be a word on its own and is allowed.
 */
export const searchReady = (q: string): boolean => {
  const t = q.trim();
  if (!t) return false;
  const chars = [...t];
  // Two characters, or a single character outside ASCII (a CJK/Arabic word).
  return chars.length >= 2 || /[^\u0000-\u007f]/.test(chars[0] ?? "");
};
