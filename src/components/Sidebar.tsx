import { createSignal, For, Show } from "solid-js";
import Icon, { type IconName } from "./Icon";
import { labelName, SIDEBAR_CATEGORIES, SIDEBAR_SYSTEM } from "../lib/format";
import type { Category, CategoryCount, Label } from "../lib/types";

const ICONS: Record<string, IconName> = {
  INBOX: "inbox",
  STARRED: "star",
  IMPORTANT: "important",
  SENT: "send",
  DRAFT: "draft",
  SPAM: "spam",
  TRASH: "trash",
};

interface Props {
  labels: Label[];
  categoryCounts?: CategoryCount[];
  current: string;
  category?: Category | null;
  collapsed: boolean;
  onSelect(id: string): void;
  onSelectCategory?(cat: Category): void;
  onCompose(): void;
}

export default function Sidebar(p: Props) {
  const [showCategories, setShowCategories] = createSignal(true);
  const [showLabels, setShowLabels] = createSignal(true);

  const system = () =>
    SIDEBAR_SYSTEM.map((id) => p.labels.find((l) => l.id === id) ?? { id, name: id, kind: "system" as const, unread: 0, total: 0 });

  const categories = () =>
    SIDEBAR_CATEGORIES.map((c) => {
      const cnt = p.categoryCounts?.find((x) => x.id === c.category);
      const lbl = p.labels.find((l) => l.id === c.id);
      return {
        id: c.id,
        category: c.category,
        name: c.name,
        icon: c.icon as IconName,
        unread: cnt?.unread ?? lbl?.unread ?? 0,
      };
    });

  const user = () => p.labels.filter((l) => l.kind === "user");

  const Item = (props: {
    label: { id: string; name: string; unread: number };
    icon: IconName;
    active?: boolean;
    onClick(): void;
    showUnreadBadge?: boolean;
  }) => {
    const isAct = () => props.active ?? p.current === props.label.id;
    const showCount = () => (props.showUnreadBadge ?? true) && props.label.unread > 0;
    return (
      <button
        class={`flex h-8 w-full items-center gap-4 rounded-r-full pr-3 text-left text-sm ${p.collapsed ? "justify-center pl-0 rounded-full" : "pl-6"} ${
          isAct() ? "bg-primary-container font-semibold text-on-primary-container" : "text-text hover:bg-hover"
        } ${showCount() ? "font-semibold" : ""}`}
        onClick={props.onClick}
        title={labelName(props.label.id, props.label.name)}
        aria-current={isAct() ? "page" : undefined}
      >
        <Icon name={props.icon} size={20} />
        <Show when={!p.collapsed}>
          <span class="flex-1 truncate">{labelName(props.label.id, props.label.name)}</span>
          <Show when={showCount()}>
            <span class="rounded-full bg-primary/10 px-1.5 py-0.5 text-xs text-primary dark:bg-primary/20">{props.label.unread}</span>
          </Show>
        </Show>
      </button>
    );
  };

  return (
    <nav class={`flex shrink-0 flex-col overflow-y-auto pb-4 pr-2 ${p.collapsed ? "w-[72px] pl-2" : "w-[256px]"}`} aria-label="Labels">
      <div class={`mb-4 mt-1 ${p.collapsed ? "" : "pl-2"}`}>
        <button
          class={`flex h-14 items-center gap-3 rounded-2xl bg-primary-container text-sm font-medium text-on-primary-container shadow-sm hover:shadow-md ${
            p.collapsed ? "w-14 justify-center" : "px-4 pr-6"
          }`}
          title="Compose (c)"
          onClick={() => p.onCompose()}
        >
          <Icon name="pencil" size={22} />
          <Show when={!p.collapsed}>Compose</Show>
        </button>
      </div>

      {/* Main Mailbox folders */}
      <For each={system()}>
        {(l) => (
          <Item
            label={l}
            icon={ICONS[l.id] ?? "label"}
            active={p.current === l.id && (l.id !== "INBOX" || !p.category || p.category === "CATEGORY_PERSONAL")}
            onClick={() => p.onSelect(l.id)}
            showUnreadBadge={["INBOX", "SPAM", "DRAFT"].includes(l.id)}
          />
        )}
      </For>

      {/* Gmail Categories */}
      <div class="mt-3">
        <Show when={!p.collapsed}>
          <button
            class="flex w-full items-center justify-between px-6 py-1.5 text-xs font-semibold uppercase tracking-wider text-muted hover:text-text"
            onClick={() => setShowCategories((v) => !v)}
          >
            <span>Categories</span>
            <Icon name="chevronDown" class={`transition-transform ${showCategories() ? "" : "-rotate-90"}`} size={14} />
          </button>
        </Show>
        <Show when={showCategories() || p.collapsed}>
          <For each={categories()}>
            {(c) => (
              <Item
                label={{ id: c.id, name: c.name, unread: c.unread }}
                icon={c.icon}
                active={p.current === "INBOX" && p.category === c.category}
                onClick={() => {
                  if (p.onSelectCategory) p.onSelectCategory(c.category);
                  else p.onSelect(c.id);
                }}
              />
            )}
          </For>
        </Show>
      </div>

      {/* User Custom Labels */}
      <Show when={user().length > 0}>
        <div class="mt-3">
          <Show when={!p.collapsed}>
            <button
              class="flex w-full items-center justify-between px-6 py-1.5 text-xs font-semibold uppercase tracking-wider text-muted hover:text-text"
              onClick={() => setShowLabels((v) => !v)}
            >
              <span>Labels</span>
              <Icon name="chevronDown" class={`transition-transform ${showLabels() ? "" : "-rotate-90"}`} size={14} />
            </button>
          </Show>
          <Show when={showLabels() || p.collapsed}>
            <For each={user()}>
              {(l) => (
                <Item
                  label={l}
                  icon="label"
                  active={p.current === l.id}
                  onClick={() => p.onSelect(l.id)}
                />
              )}
            </For>
          </Show>
        </div>
      </Show>
    </nav>
  );
}
