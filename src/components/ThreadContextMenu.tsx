import { onCleanup, onMount, Show, For } from "solid-js";
import Icon, { type IconName } from "./Icon";
import { snack } from "./Snackbar";
import { CATEGORY_TABS } from "./InboxTabs";
import type { ThreadRow } from "../lib/types";
import type { RowActions } from "./ThreadList";

interface Props {
  x: number;
  y: number;
  thread: ThreadRow;
  inInbox: boolean;
  actions: RowActions;
  onClose(): void;
  onCopyCode?(code: string): void;
}

export default function ThreadContextMenu(p: Props) {
  let menuEl!: HTMLDivElement;

  onMount(() => {
    const onDown = (e: PointerEvent) => {
      if (menuEl && !menuEl.contains(e.target as Node)) {
        p.onClose();
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        p.onClose();
      }
    };
    const onScroll = () => p.onClose();

    document.addEventListener("pointerdown", onDown, false);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("scroll", onScroll, true);

    // Viewport bounding
    const rect = menuEl.getBoundingClientRect();
    const pad = 12;
    let posX = p.x;
    let posY = p.y;

    if (posX + rect.width > window.innerWidth - pad) {
      posX = Math.max(pad, window.innerWidth - rect.width - pad);
    }
    if (posY + rect.height > window.innerHeight - pad) {
      posY = Math.max(pad, window.innerHeight - rect.height - pad);
    }
    menuEl.style.left = `${posX}px`;
    menuEl.style.top = `${posY}px`;

    onCleanup(() => {
      document.removeEventListener("pointerdown", onDown, false);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("scroll", onScroll, true);
    });
  });

  const exec = (fn: (t: ThreadRow) => void) => {
    const thread = { ...p.thread };
    try {
      fn(thread);
    } finally {
      p.onClose();
    }
  };

  const copyText = (text: string, msg: string) => {
    navigator.clipboard?.writeText(text);
    p.onClose();
    snack(msg);
  };

  const MenuItem = (item: {
    icon: IconName;
    label: string;
    shortcut?: string;
    danger?: boolean;
    active?: boolean;
    onClick(t: ThreadRow): void;
  }) => (
    <button
      type="button"
      class={`flex h-8 w-full items-center gap-2.5 rounded-lg px-2.5 text-xs text-left transition-colors ${
        item.danger
          ? "text-danger hover:bg-danger/10"
          : item.active
          ? "text-text hover:bg-hover font-medium"
          : "text-text hover:bg-hover"
      }`}
      onClick={(e) => {
        e.stopPropagation();
        exec(item.onClick);
      }}
    >
      <Icon
        name={item.icon}
        size={16}
        class={item.active ? "text-star fill-current" : item.danger ? "text-danger" : "text-muted"}
      />
      <span class="truncate flex-1">{item.label}</span>
      <Show when={item.active}>
        <span class="size-1.5 rounded-full bg-star shadow-xs shrink-0" title="Active" />
      </Show>
      <Show when={item.shortcut}>
        <kbd class="ml-auto pl-3 font-mono text-[10px] text-faint">{item.shortcut}</kbd>
      </Show>
    </button>
  );

  return (
    <div
      ref={menuEl}
      role="menu"
      aria-label="Conversation actions"
      class="fixed z-50 min-w-[210px] max-w-[280px] rounded-2xl bg-surface-high p-1.5 text-text shadow-2xl ring-1 ring-black/10 dark:ring-white/10 animate-pop-in"
      style={{ left: `${p.x}px`, top: `${p.y}px` }}
      onClick={(e) => e.stopPropagation()}
      onContextMenu={(e) => e.preventDefault()}
    >
      {/* Mark Read/Unread */}
      <MenuItem
        icon={p.thread.unread ? "markRead" : "markUnread"}
        label={p.thread.unread ? "Mark as read" : "Mark as unread"}
        shortcut={p.thread.unread ? "Shift+I" : "Shift+U"}
        onClick={(t) => p.actions.toggleRead(t)}
      />

      {/* Star */}
      <MenuItem
        icon={p.thread.starred ? "starFilled" : "star"}
        label={p.thread.starred ? "Remove star" : "Star"}
        shortcut="s"
        active={p.thread.starred}
        onClick={(t) => p.actions.toggleStar(t)}
      />

      {/* Important */}
      <MenuItem
        icon="important"
        label={p.thread.important ? "Mark as not important" : "Mark as important"}
        active={p.thread.important}
        onClick={(t) => p.actions.toggleImportant(t)}
      />

      {/* Archive */}
      <Show when={p.actions.archive}>
        <MenuItem
          icon="archive"
          label="Archive"
          shortcut="e"
          onClick={(t) => p.actions.archive!(t)}
        />
      </Show>

      {/* Trash */}
      <MenuItem
        icon="trash"
        label="Move to trash"
        shortcut="#"
        danger
        onClick={(t) => p.actions.trash(t)}
      />

      {/* Spam */}
      <MenuItem
        icon="spam"
        label="Report spam"
        shortcut="!"
        danger
        onClick={(t) => p.actions.markSpam(t)}
      />

      {/* Move to Category */}
      <Show when={p.actions.moveToCategory}>
        <div class="my-1.5 h-px bg-divider" />
        <div class="px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wider text-faint">
          Move to Tab
        </div>
        <For each={CATEGORY_TABS}>
          {(c) => (
            <button
              type="button"
              class="flex h-7 w-full items-center gap-2 rounded-lg px-2.5 text-xs text-left text-muted hover:bg-hover hover:text-text transition-colors"
              onClick={(e) => {
                e.stopPropagation();
                exec((t) => p.actions.moveToCategory!(t, c.id));
              }}
            >
              <Icon name={c.icon} size={14} />
              <span>{c.name}</span>
            </button>
          )}
        </For>
      </Show>

      {/* Quick Copy Info */}
      <div class="my-1.5 h-px bg-divider" />
      <Show when={p.thread.otpCode}>
        <MenuItem
          icon="copy"
          label={`Copy Code: ${p.thread.otpCode}`}
          shortcut="c"
          onClick={() => {
            if (p.onCopyCode) p.onCopyCode(p.thread.otpCode!);
            else copyText(p.thread.otpCode!, "Verification code copied");
          }}
        />
      </Show>
      <MenuItem
        icon="copy"
        label="Copy subject"
        onClick={() => copyText(p.thread.subject || "(no subject)", "Subject copied to clipboard")}
      />
      <MenuItem
        icon="copy"
        label="Copy thread ID"
        onClick={() => copyText(p.thread.threadId, "Thread ID copied to clipboard")}
      />
    </div>
  );
}
