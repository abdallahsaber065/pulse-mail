import { For, Show } from "solid-js";
import Icon, { type IconName } from "./Icon";
import { labelName, SIDEBAR_SYSTEM } from "../lib/format";
import type { Label } from "../lib/types";

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
  current: string;
  collapsed: boolean;
  onSelect(id: string): void;
}

export default function Sidebar(p: Props) {
  const system = () =>
    SIDEBAR_SYSTEM.map((id) => p.labels.find((l) => l.id === id) ?? { id, name: id, kind: "system" as const, unread: 0, total: 0 });
  const user = () => p.labels.filter((l) => l.kind === "user");

  const Item = (props: { label: Label; icon: IconName }) => {
    const active = () => p.current === props.label.id;
    const showCount = () => props.label.unread > 0 && ["INBOX", "SPAM"].includes(props.label.id) || (props.label.kind === "user" && props.label.unread > 0);
    return (
      <button
        class={`flex h-8 w-full items-center gap-4 rounded-r-full pr-3 text-left text-sm ${p.collapsed ? "justify-center pl-0 rounded-full" : "pl-6"} ${
          active() ? "bg-primary-container font-semibold text-on-primary-container" : "text-text hover:bg-hover"
        } ${showCount() ? "font-semibold" : ""}`}
        onClick={() => p.onSelect(props.label.id)}
        title={labelName(props.label.id, props.label.name)}
        aria-current={active() ? "page" : undefined}
      >
        <Icon name={props.icon} size={20} />
        <Show when={!p.collapsed}>
          <span class="flex-1 truncate">{labelName(props.label.id, props.label.name)}</span>
          <Show when={showCount()}>
            <span class="text-xs">{props.label.unread}</span>
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
          title="Compose is coming in a later phase"
          disabled
        >
          <Icon name="pencil" size={22} />
          <Show when={!p.collapsed}>Compose</Show>
        </button>
      </div>
      <For each={system()}>{(l) => <Item label={l} icon={ICONS[l.id] ?? "label"} />}</For>
      <Show when={user().length > 0}>
        <div class={`mb-1 mt-4 text-base font-medium text-text ${p.collapsed ? "hidden" : "pl-6"}`}>Labels</div>
        <For each={user()}>{(l) => <Item label={l} icon="label" />}</For>
      </Show>
    </nav>
  );
}
