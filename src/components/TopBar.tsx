import { Show } from "solid-js";
import Icon from "./Icon";
import Avatar from "./Avatar";
import { theme } from "../lib/theme";
import type { SyncStatus } from "../lib/types";

interface Props {
  query: string;
  onQuery(q: string): void;
  onToggleSidebar(): void;
  onRefresh(): void;
  onHelp(): void;
  onSignOut(): void;
  account: string | null;
  sync: SyncStatus | null;
  searchRef(el: HTMLInputElement): void;
}

export default function TopBar(p: Props) {
  const syncLabel = () => {
    const s = p.sync;
    if (!s || !s.running) return "Not syncing";
    if (s.error) return s.error;
    if (s.syncing) return "Checking for mail…";
    return s.imapConnected ? "Live: new mail arrives instantly" : "Checking every minute";
  };
  const dotClass = () => {
    const s = p.sync;
    if (!s || !s.running) return "bg-outline";
    if (s.error) return "bg-danger";
    return s.imapConnected ? "bg-otp" : "bg-star";
  };

  return (
    <header class="flex h-16 shrink-0 items-center gap-2 px-2">
      <div class="flex w-[232px] shrink-0 items-center gap-1">
        <button class="grid size-12 place-items-center rounded-full text-muted hover:bg-hover" onClick={p.onToggleSidebar} title="Main menu">
          <Icon name="menu" size={24} />
        </button>
        <div class="flex items-center gap-2 pl-1">
          <div class="grid size-8 place-items-center rounded-lg bg-primary text-on-primary">
            <Icon name="mail" size={20} />
          </div>
          <span class="font-display text-[22px] text-muted">Pulse</span>
        </div>
      </div>

      <label class="flex h-12 max-w-[720px] flex-1 items-center gap-2 rounded-full bg-search px-2 focus-within:bg-surface focus-within:shadow-md">
        <span class="grid size-10 place-items-center text-muted">
          <Icon name="search" size={22} />
        </span>
        <input
          ref={p.searchRef}
          class="selectable h-full flex-1 bg-transparent text-base text-text outline-none placeholder:text-muted"
          placeholder="Search mail"
          value={p.query}
          onInput={(e) => p.onQuery(e.currentTarget.value)}
          spellcheck={false}
          aria-label="Search mail"
        />
        <Show when={p.query}>
          <button class="grid size-10 place-items-center rounded-full text-muted hover:bg-hover" onClick={() => p.onQuery("")} title="Clear search">
            <Icon name="close" />
          </button>
        </Show>
      </label>

      <div class="ml-auto flex items-center gap-1">
        <div class="mr-2 flex items-center gap-2 text-xs text-muted" title={syncLabel()}>
          <span class={`size-2 rounded-full ${dotClass()}`} />
          <span class="hidden max-w-56 truncate xl:inline">{syncLabel()}</span>
        </div>
        <button class="grid size-10 place-items-center rounded-full text-muted hover:bg-hover" onClick={p.onRefresh} title="Check mail now (Shift+R)">
          <Icon name="refresh" />
        </button>
        <button class="grid size-10 place-items-center rounded-full text-muted hover:bg-hover" onClick={() => theme.cycle()} title={`Theme: ${theme.pref()}`}>
          <Icon name={theme.pref() === "dark" ? "moon" : theme.pref() === "light" ? "sun" : "auto"} />
        </button>
        <button class="grid size-10 place-items-center rounded-full text-muted hover:bg-hover" onClick={p.onHelp} title="Keyboard shortcuts (?)">
          <Icon name="keyboard" />
        </button>
        <Show when={p.account}>
          <button class="ml-1 rounded-full p-1 hover:bg-hover" title={`${p.account}\nClick to sign out`} onClick={p.onSignOut}>
            <Avatar name={p.account!} email={p.account!} size={32} />
          </button>
        </Show>
      </div>
    </header>
  );
}
