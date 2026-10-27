import { createEffect, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import Icon from "./Icon";
import AccountMenu from "./AccountMenu";
import SyncIndicator from "./SyncIndicator";
import SearchSyntaxGuide from "./SearchSyntaxGuide";
import { applySuggestion, getSearchSuggestions } from "../lib/search-syntax";
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
  /** Tiny "Demo" chip; omitted from screenshot/GIF captures (`?shot=1`). */
  demo?: boolean;
}

export default function TopBar(p: Props) {
  let searchInputEl!: HTMLInputElement;
  let containerEl!: HTMLDivElement;
  const [showGuide, setShowGuide] = createSignal(false);
  const [focused, setFocused] = createSignal(false);
  const [selectedIdx, setSelectedIdx] = createSignal(0);

  const suggestions = () => getSearchSuggestions(p.query, searchInputEl?.selectionStart ?? undefined);

  // Reset highlight when suggestions list changes
  createEffect(() => {
    suggestions();
    setSelectedIdx(0);
  });

  const chooseSuggestion = (token: string) => {
    const curPos = searchInputEl?.selectionStart ?? undefined;
    const { newQuery, newCursor } = applySuggestion(p.query, token, curPos);
    p.onQuery(newQuery);
    setShowGuide(false);
    setTimeout(() => {
      searchInputEl?.focus();
      searchInputEl?.setSelectionRange(newCursor, newCursor);
    }, 10);
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (showGuide()) {
      if (e.key === "Escape") {
        setShowGuide(false);
        e.preventDefault();
      }
      return;
    }

    const list = suggestions();
    if (!focused() || list.length === 0) return;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIdx((prev) => (prev + 1) % list.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIdx((prev) => (prev - 1 + list.length) % list.length);
    } else if (e.key === "Enter" || e.key === "Tab") {
      if (selectedIdx() >= 0 && selectedIdx() < list.length) {
        const item = list[selectedIdx()];
        if (p.query.trim().toLowerCase() !== item.token.toLowerCase()) {
          e.preventDefault();
          chooseSuggestion(item.token);
        }
      }
    } else if (e.key === "Escape") {
      setFocused(false);
    }
  };

  onMount(() => {
    const onDocClick = (e: MouseEvent) => {
      if (containerEl && !containerEl.contains(e.target as Node)) {
        setShowGuide(false);
        setFocused(false);
      }
    };
    window.addEventListener("pointerdown", onDocClick);
    onCleanup(() => window.removeEventListener("pointerdown", onDocClick));
  });

  return (
    <header class="flex h-16 shrink-0 items-center gap-2 px-2">
      <div class="flex w-[232px] shrink-0 items-center gap-1">
        <button class="grid size-12 place-items-center rounded-full text-muted hover:bg-hover" onClick={p.onToggleSidebar} title="Main menu">
          <Icon name="menu" size={24} />
        </button>
        <div class="flex items-center gap-2 pl-1 select-none">
          <img src="/icon.png" alt="Pulse Mail logo" class="size-8 rounded-lg shadow-xs select-none object-contain" />
          <span class="font-display text-[22px] text-muted">Pulse</span>
          <Show when={p.demo}>
            <span class="rounded-full bg-otp-container px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-otp">Demo</span>
          </Show>
        </div>
      </div>

      {/* Search Input Container with Autocomplete and Syntax Guide */}
      <div
        ref={containerEl}
        class="relative flex h-12 max-w-[720px] flex-1 items-center gap-1.5 rounded-full bg-search px-3 focus-within:bg-surface focus-within:shadow-md transition-shadow"
      >
        <span class="grid size-8 place-items-center text-muted">
          <Icon name="search" size={20} />
        </span>
        <input
          ref={(el) => {
            searchInputEl = el;
            p.searchRef(el);
          }}
          class="selectable h-full flex-1 bg-transparent text-base text-text outline-none placeholder:text-muted"
          placeholder="Search mail (e.g. is:unread, from:google, category:primary)"
          value={p.query}
          onInput={(e) => {
            p.onQuery(e.currentTarget.value);
            setShowGuide(false);
          }}
          onFocus={() => setFocused(true)}
          onKeyDown={handleKeyDown}
          spellcheck={false}
          aria-label="Search mail"
        />
        <Show when={p.query}>
          <button
            class="grid size-8 place-items-center rounded-full text-muted hover:bg-hover hover:text-text"
            onClick={() => {
              p.onQuery("");
              searchInputEl?.focus();
            }}
            title="Clear search"
          >
            <Icon name="close" size={16} />
          </button>
        </Show>

        {/* Search Syntax Info Icon */}
        <button
          type="button"
          class={`grid size-8 place-items-center rounded-full transition-colors ${
            showGuide() ? "bg-primary text-on-primary" : "text-muted hover:bg-hover hover:text-text"
          }`}
          onClick={(e) => {
            e.stopPropagation();
            setShowGuide((g) => !g);
          }}
          title="Search syntax guide (is:, from:, has:, category:, etc.)"
          aria-label="Search syntax guide"
        >
          <Icon name="info" size={18} />
        </button>

        {/* Autocomplete Suggestions Dropdown */}
        <Show when={focused() && !showGuide() && suggestions().length > 0}>
          <div
            class="absolute left-0 right-0 top-full mt-1.5 z-50 max-h-72 overflow-y-auto rounded-2xl border border-divider bg-surface-high p-1.5 shadow-2xl animate-fade-in"
            role="listbox"
            aria-label="Search suggestions"
            onPointerDown={(e) => e.stopPropagation()}
          >
            <div class="px-2.5 py-1 text-[10px] font-bold tracking-wider uppercase text-faint border-b border-divider mb-1">
              Suggestions & Operators
            </div>
            <For each={suggestions()}>
              {(item, idx) => (
                <button
                  type="button"
                  class={`flex w-full items-center gap-3 rounded-xl px-3 py-2 text-left transition-colors ${
                    selectedIdx() === idx() ? "bg-primary/10 text-primary font-medium" : "hover:bg-hover text-text"
                  }`}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    chooseSuggestion(item.token);
                  }}
                >
                  <span class="font-mono text-xs font-semibold px-2 py-0.5 rounded bg-surface-container text-primary">
                    {item.syntax}
                  </span>
                  <span class="flex-1 truncate text-xs text-muted">
                    {item.description}
                  </span>
                  <span class="text-[11px] text-faint font-mono hidden sm:inline">
                    {item.example}
                  </span>
                </button>
              )}
            </For>
          </div>
        </Show>

        {/* Full Search Syntax Guide Modal/Popover */}
        <Show when={showGuide()}>
          <SearchSyntaxGuide
            onSelect={(token) => chooseSuggestion(token)}
            onClose={() => setShowGuide(false)}
          />
        </Show>
      </div>

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
