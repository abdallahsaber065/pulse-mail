import { createVirtualizer } from "@tanstack/solid-virtual";
import { createEffect, For, Show } from "solid-js";
import Icon from "./Icon";
import { listDate } from "../lib/format";
import type { ThreadRow } from "../lib/types";

export const ROW_HEIGHT = 40;

interface Props {
  threads: ThreadRow[];
  selectedId: string | null;
  cursor: number;
  loading: boolean;
  emptyText: string;
  onOpen(t: ThreadRow, index: number): void;
  onCopyCode(code: string): void;
}

export default function ThreadList(p: Props) {
  let scrollEl!: HTMLDivElement;
  const virtualizer = createVirtualizer({
    get count() {
      return p.threads.length;
    },
    getScrollElement: () => scrollEl,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });

  // Keep the keyboard cursor in view.
  createEffect(() => {
    if (p.cursor >= 0 && p.cursor < p.threads.length) virtualizer.scrollToIndex(p.cursor, { align: "auto" });
  });

  return (
    <div ref={scrollEl} class="relative h-full overflow-y-auto" role="list" aria-label="Conversations">
      <Show
        when={p.threads.length > 0}
        fallback={
          <div class="grid h-full place-items-center p-8 text-center text-sm text-muted">{p.loading ? "Loading…" : p.emptyText}</div>
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
                    class={`group absolute left-0 right-0 flex cursor-pointer items-center gap-3 border-b border-divider pl-4 pr-3 text-sm ${
                      selected() ? "bg-selected" : t().unread ? "bg-surface" : "bg-surface-dim"
                    } hover:z-10 hover:shadow-[inset_1px_0_0_var(--outline),inset_-1px_0_0_var(--outline),0_1px_2px_rgba(0,0,0,.25)] ${
                      focused() ? "shadow-[inset_3px_0_0_var(--primary)]" : ""
                    }`}
                    style={{ top: `${item.start}px`, height: `${ROW_HEIGHT}px` }}
                    onClick={() => p.onOpen(t(), item.index)}
                  >
                    <span class={t().starred ? "text-star" : "text-outline"}>
                      <Icon name={t().starred ? "starFilled" : "star"} size={18} />
                    </span>
                    <span class={`w-44 shrink-0 truncate ${t().unread ? "font-bold text-text" : "text-text"}`}>
                      {t().fromName || t().fromEmail}
                      <Show when={t().messageCount > 1}>
                        <span class="ml-1 text-xs font-normal text-faint">{t().messageCount}</span>
                      </Show>
                    </span>
                    <span class="flex min-w-0 flex-1 items-center gap-2">
                      <Show when={t().otpCode}>
                        <button
                          class="shrink-0 rounded-md bg-otp-container px-2 py-0.5 font-mono text-xs font-semibold tracking-wider text-otp hover:brightness-95"
                          title="Copy verification code"
                          onClick={(e) => {
                            e.stopPropagation();
                            p.onCopyCode(t().otpCode!);
                          }}
                        >
                          {t().otpCode}
                        </button>
                      </Show>
                      <span class="truncate">
                        <span class={t().unread ? "font-bold" : ""}>{t().subject || "(no subject)"}</span>
                        <span class="text-faint"> - {t().snippet}</span>
                      </span>
                    </span>
                    <span class={`w-20 shrink-0 text-right text-xs ${t().unread ? "font-bold text-text" : "text-faint"}`}>
                      {listDate(t().internalDate)}
                    </span>
                  </div>
                </Show>
              );
            }}
          </For>
        </div>
      </Show>
    </div>
  );
}
