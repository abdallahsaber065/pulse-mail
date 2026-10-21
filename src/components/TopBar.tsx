import { Show } from "solid-js";
import Icon from "./Icon";
import AccountMenu from "./AccountMenu";
import SyncIndicator from "./SyncIndicator";
import { theme } from "../lib/theme";
import type { Account, Activity, SyncStatus } from "../lib/types";

interface Props {
  query: string;
  onQuery(q: string): void;
  onToggleSidebar(): void;
  onRefresh(): void;
  onHelp(): void;
  onSettings(): void;
  onSignOut(accountId: number): void;
  accounts: Account[];
  /** Account in view; null = all inboxes. */
  currentAccount: number | null;
  onSelectAccount(accountId: number | null): void;
  onAddAccount(): void;
  onPauseHistory(accountId: number, pause: boolean): void;
  sync: SyncStatus | null;
  online: boolean;
  activity: Activity[];
  searchRef(el: HTMLInputElement): void;
}

export default function TopBar(p: Props) {
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
        <Show when={p.accounts.length > 0}>
          <SyncIndicator
            sync={p.sync}
            online={p.online}
            activity={p.activity}
            accounts={p.accounts}
            onRetry={p.onRefresh}
            onPauseHistory={p.onPauseHistory}
          />
        </Show>
        <button class="grid size-10 place-items-center rounded-full text-muted hover:bg-hover" onClick={p.onRefresh} title="Check mail now (Shift+R)">
          <Icon name="refresh" class={p.sync?.syncing && p.sync.foreground ? "animate-spin-slow" : ""} />
        </button>
        <button class="grid size-10 place-items-center rounded-full text-muted hover:bg-hover" onClick={() => theme.cycle()} title={`Theme: ${theme.pref()}`}>
          <Icon name={theme.pref() === "dark" ? "moon" : theme.pref() === "light" ? "sun" : "auto"} />
        </button>
        <button class="grid size-10 place-items-center rounded-full text-muted hover:bg-hover" onClick={p.onSettings} title="Settings (,)">
          <Icon name="settings" />
        </button>
        <Show when={p.accounts.length > 0}>
          <AccountMenu
            accounts={p.accounts}
            current={p.currentAccount}
            live={!!p.sync?.imapConnected}
            onSelect={p.onSelectAccount}
            onAddAccount={p.onAddAccount}
            onSettings={p.onSettings}
            onShortcuts={p.onHelp}
            onSignOut={p.onSignOut}
          />
        </Show>
      </div>
    </header>
  );
}
