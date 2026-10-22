import { expect, it } from "vitest";
import { loadRows } from "./paging";

it("loads just the next page and stops at the end of a long mailbox", async () => {
  const calls: number[] = [];
  const fetch = async (offset: number, limit: number) => {
    calls.push(offset);
    return Array.from({ length: Math.min(limit, Math.max(0, 450 - offset)) }, (_, i) => offset + i);
  };
  const first = await loadRows(fetch, 200);
  const second = await loadRows(fetch, 400, first);
  expect(calls).toEqual([0, 200]);
  expect(second).toHaveLength(400);
  expect(first).toHaveLength(200);
  const last = await loadRows(fetch, 600, second);
  expect(last).toHaveLength(450);
  expect(new Set(last).size).toBe(450);
  expect(calls).toEqual([0, 200, 400]);
});
