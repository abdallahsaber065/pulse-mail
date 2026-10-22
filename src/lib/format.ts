// Pure formatting helpers (unit-tested in format.test.ts).

const SYSTEM_LABELS: Record<string, string> = {
  INBOX: "Inbox",
  STARRED: "Starred",
  IMPORTANT: "Important",
  SENT: "Sent",
  DRAFT: "Drafts",
  SPAM: "Spam",
  TRASH: "Trash",
  UNREAD: "Unread",
  CHAT: "Chats",
  CATEGORY_PERSONAL: "Primary",
  CATEGORY_SOCIAL: "Social",
  CATEGORY_PROMOTIONS: "Promotions",
  CATEGORY_UPDATES: "Updates",
  CATEGORY_FORUMS: "Forums",
};

/** Gmail-ordered system labels shown in the sidebar. */
export const SIDEBAR_SYSTEM = ["INBOX", "STARRED", "IMPORTANT", "SENT", "DRAFT", "SPAM", "TRASH"];

export function labelName(id: string, name: string): string {
  return SYSTEM_LABELS[id] ?? name;
}

/** Gmail-style list date: time today, "Oct 3" this year, "3/10/24" older. */
export function listDate(ms: number, now = Date.now()): string {
  const d = new Date(ms);
  const n = new Date(now);
  if (d.toDateString() === n.toDateString()) {
    return d.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
  }
  if (d.getFullYear() === n.getFullYear()) {
    return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  }
  return d.toLocaleDateString(undefined, { year: "2-digit", month: "numeric", day: "numeric" });
}

/** Full date for the reader header, plus a relative hint. */
export function fullDate(ms: number, now = Date.now()): string {
  const d = new Date(ms);
  const abs = d.toLocaleString(undefined, { weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
  return `${abs} (${relative(ms, now)})`;
}

export function relative(ms: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - ms) / 1000));
  if (s < 60) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m} minute${m === 1 ? "" : "s"} ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} hour${h === 1 ? "" : "s"} ago`;
  const days = Math.round(h / 24);
  return `${days} day${days === 1 ? "" : "s"} ago`;
}

export function initials(name: string, email = ""): string {
  // A bare address never yields initials from its domain ("abdallah@gmail.com"
  // is "A", not "AG"): only a real name gives two letters.
  const isAddress = (s: string) => s.includes("@");
  const real = name.trim() && !isAddress(name) ? name.trim() : "";
  const src = real || (name || email).trim().split("@")[0];
  if (!src) return "?";
  const parts = src
    .replace(/[<>"()]/g, "")
    .split(/[\s._-]+/)
    .map((p) => p.replace(/^[^\p{L}\p{N}]+/u, ""))
    .filter(Boolean);
  const first = parts[0]?.[0] ?? "?";
  const second = parts.length > 1 ? parts[parts.length - 1][0] : "";
  return (first + second).toUpperCase();
}

/** Stable avatar hue from an address. */
export function avatarHue(key: string): number {
  let h = 0;
  for (let i = 0; i < key.length; i++) h = (h * 31 + key.charCodeAt(i)) >>> 0;
  return h % 360;
}

/** 1536 -> "1.5 KB". */
export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}
