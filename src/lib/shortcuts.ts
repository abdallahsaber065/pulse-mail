// Gmail-compatible keyboard shortcuts (subset). See docs/DESIGN.md.

export interface ShortcutHandlers {
  next(): void;
  prev(): void;
  open(): void;
  back(): void;
  focusSearch(): void;
  goto(labelId: string): void;
  copyCode(): void;
  refresh(): void;
  toggleHelp(): void;
  archive(): void;
  star(): void;
  markRead(): void;
  markUnread(): void;
  settings(): void;
}

export const SHORTCUTS: [string, string][] = [
  ["j / k", "Next / previous conversation"],
  ["Enter or o", "Open conversation"],
  ["u or Esc", "Back to list"],
  ["/", "Search mail"],
  ["c", "Copy verification code"],
  ["e", "Archive"],
  ["s", "Star / unstar"],
  ["Shift + i", "Mark as read"],
  ["Shift + u", "Mark as unread"],
  ["g then i", "Go to Inbox"],
  ["g then s", "Go to Starred"],
  ["g then t", "Go to Sent"],
  ["g then d", "Go to Drafts"],
  ["Shift + r", "Check mail now"],
  [",", "Settings"],
  ["Ctrl + k", "Command palette"],
  ["?", "Show keyboard shortcuts"],
];

const GOTO: Record<string, string> = { i: "INBOX", s: "STARRED", t: "SENT", d: "DRAFT" };

export function installShortcuts(h: ShortcutHandlers): () => void {
  let pendingG = 0;
  const onKey = (e: KeyboardEvent) => {
    const target = e.target as HTMLElement | null;
    const typing = target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);
    if (typing) {
      if (e.key === "Escape") target!.blur();
      return;
    }
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    // A dialog, sheet or popover owns the keyboard while open.
    if (document.querySelector('[aria-modal="true"]')) return;

    if (pendingG && Date.now() - pendingG < 1200 && GOTO[e.key]) {
      pendingG = 0;
      e.preventDefault();
      h.goto(GOTO[e.key]);
      return;
    }
    pendingG = 0;

    switch (e.key) {
      case "j": h.next(); break;
      case "k": h.prev(); break;
      case "Enter":
      case "o": h.open(); break;
      case "u":
      case "Escape": h.back(); break;
      case "/": h.focusSearch(); break;
      case "c": h.copyCode(); break;
      case "R": h.refresh(); break;
      case "e": h.archive(); break;
      case "s": h.star(); break;
      case "I": h.markRead(); break;
      case "U": h.markUnread(); break;
      case ",": h.settings(); break;
      case "?": h.toggleHelp(); break;
      case "g": pendingG = Date.now(); return;
      default: return;
    }
    e.preventDefault();
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
