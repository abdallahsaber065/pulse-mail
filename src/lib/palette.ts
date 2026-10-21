// Command palette (Ctrl+K): command model and fuzzy ranking (pure).

export interface Command {
  id: string;
  group: string;
  title: string;
  hint?: string;
  icon?: string;
  keys?: string;
  /** Extra words that should match (synonyms). */
  words?: string;
  run(): void;
}

/**
 * Score how well `query` matches `text`: 0 = no match. Prefers whole-word
 * prefixes, then substring, then in-order subsequence.
 */
export function score(query: string, text: string): number {
  const q = query.trim().toLowerCase();
  if (!q) return 1;
  const t = text.toLowerCase();
  if (t.startsWith(q)) return 100 - Math.min(t.length - q.length, 50) / 10;
  if (t.split(/[\s·/–-]+/).some((w) => w.startsWith(q))) return 80;
  const i = t.indexOf(q);
  if (i >= 0) return 60 - Math.min(i, 30) / 10;
  // All query words appear (any order)?
  const words = q.split(/\s+/);
  if (words.length > 1 && words.every((w) => t.includes(w))) return 50;
  // Subsequence (e.g. "mrk rd" → "mark as read").
  let j = 0;
  for (const ch of t) if (ch === q[j]) j++;
  return j === q.length ? 20 : 0;
}

export function rank(query: string, commands: Command[], max = 40): Command[] {
  return commands
    .map((c, i) => ({ c, i, s: Math.max(score(query, c.title), score(query, `${c.group} ${c.words ?? ""}`) * 0.8) }))
    .filter((x) => x.s > 0)
    .sort((a, b) => b.s - a.s || a.i - b.i)
    .slice(0, max)
    .map((x) => x.c);
}
