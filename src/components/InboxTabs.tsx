import { For, Show } from "solid-js";
import type { Category, CategoryCount } from "../lib/types";
import Icon, { type IconName } from "./Icon";

export const CATEGORY_TABS: { id: Category; name: string; icon: IconName }[] = [
  { id: "CATEGORY_PERSONAL", name: "Primary", icon: "inbox" },
  { id: "CATEGORY_PROMOTIONS", name: "Promotions", icon: "label" },
  { id: "CATEGORY_SOCIAL", name: "Social", icon: "person" },
  { id: "CATEGORY_UPDATES", name: "Updates", icon: "info" },
  { id: "CATEGORY_FORUMS", name: "Forums", icon: "mail" },
];

export interface InboxTabItem {
  id: Category | null;
  name: string;
  icon: IconName;
}

/** Gmail-style inbox tabs with an "All" tab in the first position. Empty category tabs (other than Primary) are hidden. */
export function InboxTabs(p: {
  counts: CategoryCount[];
  current: Category | null;
  onSelect: (c: Category | null) => void;
}) {
  const count = (id: Category) => p.counts.find((c) => c.id === id);
  const totalUnread = () => p.counts.reduce((sum, c) => sum + (c.unread || 0), 0);
  const visibleCategories = () =>
    CATEGORY_TABS.filter((t) => t.id === "CATEGORY_PERSONAL" || (count(t.id)?.total ?? 0) > 0);

  const tabs = (): InboxTabItem[] => [
    { id: null, name: "All", icon: "allInbox" },
    ...visibleCategories(),
  ];

  return (
    <div
      class="@container flex h-11 shrink-0 items-stretch gap-1 overflow-hidden border-b border-divider px-2"
      role="tablist"
      aria-label="Inbox tabs"
    >
      <For each={tabs()}>
        {(t) => {
          const active = () => p.current === t.id;
          const unread = () => (t.id === null ? totalUnread() : count(t.id)?.unread ?? 0);
          return (
            <button
              role="tab"
              aria-selected={active()}
              title={t.id === null ? "All Mail" : t.name}
              class={`relative flex min-w-0 items-center gap-2 rounded-t-lg px-3 text-sm transition-colors hover:bg-black/5 dark:hover:bg-white/5 ${
                active() ? "font-semibold text-primary" : "text-muted"
              }`}
              onClick={() => p.onSelect(t.id)}
            >
              <Icon name={t.icon} size={16} />
              <span class="hidden truncate @[28rem]:inline">{t.name}</span>
              <Show when={unread() > 0}>
                <span
                  class={`rounded-full px-1.5 text-[11px] font-semibold leading-4 ${
                    active() ? "bg-primary text-surface" : "bg-surface-dim text-muted"
                  }`}
                >
                  {unread() > 999 ? "999+" : unread()}
                </span>
              </Show>
              <Show when={active()}>
                <span class="absolute inset-x-2 bottom-0 h-[3px] rounded-t bg-primary" />
              </Show>
            </button>
          );
        }}
      </For>
    </div>
  );
}
