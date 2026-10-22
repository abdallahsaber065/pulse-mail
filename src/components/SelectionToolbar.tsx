import { createEffect, Show } from "solid-js";
import Icon from "./Icon";

export default function SelectionToolbar(p: {
  count: number; all: boolean; disabled: boolean; selecting: boolean; busy: boolean;
  progress: string; scope: string;
  onSelectAll(): void; onClear(): void; onRead(): void; onUnread?(): void; onStop(): void;
}) {
  return (
    <div class="flex min-h-11 shrink-0 flex-wrap items-center gap-2 border-b border-divider px-3 py-1.5" role="toolbar" aria-label="Conversation selection">
      <label class="grid size-8 shrink-0 cursor-pointer place-items-center rounded-full hover:bg-hover" title={p.all ? "Clear selection" : p.scope}>
        <input
          type="checkbox"
          class="size-4 accent-primary"
          aria-label="Select all conversations"
          ref={(el) => createEffect(() => { el.indeterminate = p.count > 0 && !p.all; })}
          checked={p.all && p.count > 0}
          disabled={p.disabled || p.busy || p.selecting}
          onChange={() => p.all ? p.onClear() : p.onSelectAll()}
        />
      </label>
      <span class="min-w-0 flex-1 text-xs text-muted" role="status">
        {p.selecting ? "Selecting conversations…" : p.busy ? p.progress : p.count ? `${p.count.toLocaleString()} selected` : "Select conversations"}
      </span>
      <Show when={p.count > 0 && !p.busy}>
        <button class="flex h-8 items-center gap-1.5 rounded-full px-3 text-xs font-medium text-primary hover:bg-primary/10 disabled:opacity-50" disabled={p.disabled || p.selecting} onClick={p.onRead}>
          <Icon name="markRead" size={18} /> Mark as read
        </button>
        <Show when={p.onUnread}>
          <button class="flex h-8 items-center gap-1.5 rounded-full px-3 text-xs font-medium text-primary hover:bg-primary/10 disabled:opacity-50" disabled={p.disabled || p.selecting} onClick={p.onUnread}>
            <Icon name="markUnread" size={18} /> Mark as unread
          </button>
        </Show>
        <button class="h-8 rounded-full px-3 text-xs text-muted hover:bg-hover" disabled={p.selecting} onClick={p.onClear}>Clear</button>
      </Show>
      <Show when={p.busy}>
        <button class="h-8 rounded-full px-3 text-xs font-medium text-primary hover:bg-primary/10" onClick={p.onStop}>Stop</button>
      </Show>
      <Show when={p.count > 0 && !p.busy}>
        <p class="w-full pl-1 text-xs text-muted">{p.scope}</p>
      </Show>
    </div>
  );
}
