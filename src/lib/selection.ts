import type { ThreadRow } from "./types";

export type ThreadKey = Pick<ThreadRow, "accountId" | "threadId">;
export interface SelectionInfo { id: number; total: number }
export type SelectionSource =
  | { kind: "threads"; threads: ThreadKey[] }
  | { kind: "view"; accountId: number | null; labelId: string; category: string | null; query: string };
export interface ReadBatch { nextOffset: number; done: boolean; changed: number; failed: number; error: string | null }
export interface Selection {
  all: SelectionInfo | null;
  /** Explicit keys, or unchecked exceptions in select-all mode. */
  exceptions: ReadonlySet<string>;
}
export const threadKey = (t: ThreadKey) => `${t.accountId}:${t.threadId}`;
export const emptySelection = (): Selection => ({ all: null, exceptions: new Set() });
export const selectAll = (all: SelectionInfo): Selection => ({ all, exceptions: new Set() });
export const selectedCount = (s: Selection) => s.all ? s.all.total - s.exceptions.size : s.exceptions.size;
export const isSelected = (s: Selection, key: string, inSnapshot: boolean) =>
  s.all ? inSnapshot && !s.exceptions.has(key) : s.exceptions.has(key);
export function toggleSelected(s: Selection, key: string): Selection {
  const exceptions = new Set(s.exceptions);
  if (exceptions.has(key)) exceptions.delete(key);
  else exceptions.add(key);
  return { all: s.all, exceptions };
}
