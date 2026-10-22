/** Append bounded pages; scrolling never re-fetches the rows already loaded. */
export async function loadRows<T>(fetch: (offset: number, limit: number) => Promise<T[]>, wanted: number, previous: readonly T[] = []): Promise<T[]> {
  const rows = [...previous];
  while (rows.length < wanted) {
    const limit = Math.min(200, wanted - rows.length);
    const page = await fetch(rows.length, limit);
    rows.push(...page);
    if (page.length < limit) break;
  }
  return rows;
}
