import { createVirtualizer } from "@tanstack/solid-virtual";
import { createEffect, createSignal, For, Index, on, Show } from "solid-js";
import Icon, { type IconName } from "./Icon";
import ThreadContextMenu from "./ThreadContextMenu";
import { listDate } from "../lib/format";
import { prefs } from "../lib/prefs";
import type { Category, ThreadRow } from "../lib/types";

export interface RowActions {
  archive?(t: ThreadRow): void;
  trash(t: ThreadRow): void;
  markSpam(t: ThreadRow): void;
  toggleRead(t: ThreadRow): void;
  toggleStar(t: ThreadRow): void;
  toggleImportant(t: ThreadRow): void;
  moveToCategory?(t: ThreadRow, c: Category): void;
}

interface Props {
  threads: ThreadRow[];
  selectedId: string | null;
  cursor: number;
  /** Nothing cached yet and a fetch is running: show skeleton rows. */
  loading: boolean;
  empty: { icon: IconName; title: string; hint: string };
  actions: RowActions;
  isChecked(t: ThreadRow): boolean;
  onCheck(t: ThreadRow): void;
  selectionDisabled: boolean;
  viewKey: string;
  inInbox?: boolean;
  onOpen(t: ThreadRow, index: number): void;
  onCopyCode(code: string): void;
  /** The user scrolled near the bottom: load the next page if there is one. */
  onEndReached?(): void;
}

function Skeleton() {
  const widths = [62, 48, 75, 55, 68, 42, 80, 58, 70, 50, 64, 46];
  return (
    <div aria-hidden="true">
      <Index each={widths}>
        {(w, i) => (
          <div class="flex items-center gap-3 border-b border-divider pl-4 pr-3" style={{ height: `${prefs.rowHeight()}px` }}>
            <div class="skeleton size-4 rounded" />
            <div class="skeleton h-3 w-36 rounded-full" style={{ "animation-delay": `${i * 60}ms` }} />
            <div class="skeleton h-3 rounded-full" style={{ width: `${w()}%`, "animation-delay": `${i * 60}ms` }} />
            <div class="skeleton ml-auto h-3 w-12 rounded-full" />
          </div>
        )}
      </Index>
    </div>
  );
}

export default function ThreadList(p: Props) {
  let scrollEl!: HTMLDivElement;
  const [copied, setCopied] = createSignal<string | null>(null);
  const [contextMenu, setContextMenu] = createSignal<{ x: number; y: number; thread: ThreadRow } | null>(null);
  const virtualizer = createVirtualizer({
    get count() {
      return p.threads.length;
    },
    getScrollElement: () => scrollEl,
    estimateSize: () => prefs.rowHeight(),
    overscan: 12,
  });

  // Row height changed: re-measure.
  createEffect(() => {
    prefs.rowHeight();
    virtualizer.measure();
  });

  // Keep the keyboard cursor in view.
  createEffect(on(() => p.cursor, (cursor) => {
    if (cursor >= 0 && cursor < p.threads.length) virtualizer.scrollToIndex(cursor, { align: "auto" });
  }, { defer: true }));
  // Changing filters starts at the top; adding a page keeps the scroll position.
  createEffect(on(() => p.viewKey, () => { scrollEl.scrollTop = 0; }));

  // Ask for more rows when the last rendered row is close to the end.
  createEffect(() => {
    if (p.loading) return;
    const items = virtualizer.getVirtualItems();
    const last = items[items.length - 1];
    if (last && p.threads.length >= 200 && last.index >= p.threads.length - 15) p.onEndReached?.();
  });

  const comfy = () => prefs.density() === "comfortable";

  const ActionBtn = (a: { icon: IconName; title: string; onClick(e: MouseEvent): void }) => (
    <button
      class="grid size-8 place-items-center rounded-full text-muted hover:bg-black/10 hover:text-text dark:hover:bg-white/10"
      title={a.title}
      onClick={(e) => {
        e.stopPropagation();
        a.onClick(e);
      }}
    >
      <Icon name={a.icon} size={18} />
    </button>
  );

  return (
    <div ref={scrollEl} class="relative h-full overflow-y-auto" role="list" aria-label="Conversations">
      <Show
        when={p.threads.length > 0}
        fallback={
          <Show
            when={!p.loading}
            fallback={<Skeleton />}
          >
            <div class="grid h-full place-items-center p-8 animate-fade-in">
              <div class="flex max-w-sm flex-col items-center text-center">
                <div class="mb-4 grid size-20 place-items-center rounded-full bg-surface-container text-primary">
                  <Icon name={p.empty.icon} size={40} />
                </div>
                <div class="font-display text-lg text-text">{p.empty.title}</div>
                <div class="mt-1 text-sm text-muted">{p.empty.hint}</div>
              </div>
            </div>
          </Show>
        }
      >
        <div style={{ height: `${virtualizer.getTotalSize()}px`, position: "relative" }}>
          <For each={virtualizer.getVirtualItems()}>
            {(item) => {
              const t = () => p.threads[item.index];
              const selected = () => p.selectedId === t()?.threadId;
              const focused = () => p.cursor === item.index;
              return (
                <Show when={t()}>
                  <div
                    role="listitem"
                    class={`row group absolute left-0 right-0 flex cursor-pointer items-center gap-3 border-b border-divider pl-2 pr-3 text-sm transition-colors ${
                      selected() || p.isChecked(t()) ? "bg-selected" : t().unread ? "bg-surface" : "bg-surface-dim"
                    } hover:z-10 hover:shadow-[inset_1px_0_0_var(--outline),inset_-1px_0_0_var(--outline),0_1px_2px_rgba(0,0,0,.25)] ${
                      focused() ? "shadow-[inset_3px_0_0_var(--primary)]" : ""
                    }`}
                    style={{ top: `${item.start}px`, height: `${prefs.rowHeight()}px` }}
                    onClick={() => p.onOpen(t(), item.index)}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      setContextMenu({ x: e.clientX, y: e.clientY, thread: t() });
                    }}
                  >
                    <label class="grid size-8 shrink-0 cursor-pointer place-items-center rounded-full hover:bg-hover" onClick={(e) => e.stopPropagation()}>
                      <input
                        type="checkbox"
                        class="size-4 accent-primary"
                        aria-label={`Select ${t().subject || "conversation"}`}
                        checked={p.isChecked(t())}
                        disabled={p.selectionDisabled}
                        onChange={() => p.onCheck(t())}
                      />
                    </label>
                    <button
                      class={`grid size-8 shrink-0 place-items-center rounded-full hover:bg-black/5 dark:hover:bg-white/10 ${
                        t().starred ? "text-star" : "text-outline hover:text-muted"
                      }`}
                      title={t().starred ? "Unstar (s)" : "Star (s)"}
                      onClick={(e) => {
                        e.stopPropagation();
                        p.actions.toggleStar(t());
                      }}
                    >
                      <Icon name={t().starred ? "starFilled" : "star"} size={18} class="star-pop" />
                    </button>
                    <span class={`flex w-44 shrink-0 items-center gap-1 truncate ${t().unread ? "font-bold text-text" : "text-text"}`}>
                      <Show when={t().important}>
                        <button
                          class="shrink-0 text-star hover:opacity-80"
                          title="Marked as important (click to toggle)"
                          aria-label="Important"
                          onClick={(e) => {
                            e.stopPropagation();
                            p.actions.toggleImportant(t());
                          }}
                        >
                          <Icon name="important" size={14} />
                        </button>
                      </Show>
                      <span class="truncate">{t().fromName || t().fromEmail}</span>
                      <Show when={t().messageCount > 1}>
                        <span class="ml-1 text-xs font-normal text-faint">{t().messageCount}</span>
                      </Show>
                    </span>
                    <span class={`flex min-w-0 flex-1 gap-2 ${comfy() ? "flex-col justify-center gap-0" : "items-center"}`}>
                      <span class="flex min-w-0 items-center gap-2">
                        <Show when={t().otpCode}>
                          <button
                            class={`otp-chip flex shrink-0 items-center gap-1 rounded-full px-2 py-0.5 font-mono text-xs font-semibold tracking-wider transition-colors ${
                              copied() === t().threadId ? "bg-otp text-surface" : "bg-otp-container text-otp hover:brightness-95"
                            }`}
                            title="Copy verification code (c)"
                            onClick={(e) => {
                              e.stopPropagation();
                              p.onCopyCode(t().otpCode!);
                              const id = t().threadId;
                              setCopied(id);
                              window.setTimeout(() => copied() === id && setCopied(null), 1500);
                            }}
                          >
                            <Icon name={copied() === t().threadId ? "checkCircle" : "copy"} size={13} />
                            {t().otpCode}
                          </button>
                        </Show>
                        <span class="truncate">
                          <span class={t().unread ? "font-bold" : ""}>{t().subject || "(no subject)"}</span>
                          <Show when={!comfy()}>
                            <span class="text-faint"> - {t().snippet}</span>
                          </Show>
                        </span>
                      </span>
                      <Show when={comfy()}>
                        <span class="truncate text-xs text-faint">{t().snippet}</span>
                      </Show>
                    </span>
                    <span
                      class={`w-20 shrink-0 text-right text-xs group-hover:hidden ${t().unread ? "font-bold text-text" : "text-faint"}`}
                    >
                      {listDate(t().internalDate)}
                    </span>
                    <span class="hidden shrink-0 items-center gap-0.5 group-hover:flex">
                      <Show when={p.actions.archive}>
                        <ActionBtn icon="archive" title="Archive (e)" onClick={() => p.actions.archive!(t())} />
                      </Show>
                      <ActionBtn icon="trash" title="Move to trash (#)" onClick={() => p.actions.trash(t())} />
                      <ActionBtn
                        icon={t().unread ? "markRead" : "markUnread"}
                        title={t().unread ? "Mark as read (Shift+I)" : "Mark as unread (Shift+U)"}
                        onClick={() => p.actions.toggleRead(t())}
                      />
                      <ActionBtn
                        icon="more"
                        title="More actions"
                        onClick={(e) => {
                          const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
                          setContextMenu({ x: rect.left - 170, y: rect.bottom + 4, thread: t() });
                        }}
                      />
                    </span>
                  </div>
                </Show>
              );
            }}
          </For>
        </div>
      </Show>
      <Show when={contextMenu()}>
        {(menu) => (
          <ThreadContextMenu
            x={menu().x}
            y={menu().y}
            thread={menu().thread}
            inInbox={p.inInbox ?? true}
            actions={p.actions}
            onClose={() => setContextMenu(null)}
            onCopyCode={p.onCopyCode}
          />
        )}
      </Show>
    </div>
  );
}
