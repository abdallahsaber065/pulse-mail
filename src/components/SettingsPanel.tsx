// Settings side sheet. Every control applies immediately; nothing to "save".
import { For, onCleanup, onMount, Show, type JSX } from "solid-js";
import Icon from "./Icon";
import { theme, type ThemePref } from "../lib/theme";
import { prefs, type Density, type Pane } from "../lib/prefs";
import Avatar from "./Avatar";
import { formatEta, historyFraction, phaseLabel, WINDOWS } from "../lib/accounts";
import { formatSize } from "../lib/format";
import type { Account, AppStatus, NotificationMode, PrefKey, Prefs, SyncWindow } from "../lib/types";

interface Props {
  status: AppStatus;
  onClose(): void;
  onAutostart(enabled: boolean): void;
  onCloseBehavior(quit: boolean): void;
  onNotifications(mode: NotificationMode): void;
  prefs: Prefs;
  onPref<K extends PrefKey>(key: K, value: Prefs[K]): void;
  /** Accounts with live sync status. */
  accounts: Account[];
  onAddAccount(): void;
  onRemoveAccount(accountId: number): void;
  onSyncWindow(accountId: number, window: SyncWindow): void;
  onAccountNotify(accountId: number, mode: NotificationMode): void;
  onPauseHistory(accountId: number, pause: boolean): void;
}

function AccountCard(p: {
  account: Account;
  onRemove(): void;
  onWindow(w: SyncWindow): void;
  onNotify(m: NotificationMode): void;
  onPause(pause: boolean): void;
}) {
  const h = () => p.account.sync.history;
  const frac = () => historyFraction(h());
  return (
    <div class="mb-3 rounded-2xl border border-divider p-4">
      <div class="mb-3 flex items-center gap-3">
        <Avatar name={p.account.email} email={p.account.email} size={36} />
        <div class="min-w-0 flex-1">
          <div class="selectable truncate text-sm font-medium text-text">{p.account.email}</div>
          <div class={`truncate text-xs ${p.account.sync.error ? "text-danger" : "text-muted"}`}>
            {p.account.sync.error ?? (p.account.sync.imapConnected ? "Live" : p.account.sync.running ? "Connected" : "Not syncing")}
          </div>
        </div>
        <button class="h-8 rounded-full px-3 text-xs font-medium text-danger hover:bg-danger/10" onClick={p.onRemove}>
          Remove
        </button>
      </div>
      <Show when={h() && h()!.state !== "setup"}>
        <div class="mb-3 rounded-xl bg-surface-container px-3 py-2">
          <div class="mb-1 flex items-center text-xs">
            <span class="flex-1 text-text">{h()!.state === "done" ? "Download complete" : phaseLabel(h()!)}</span>
            <span class="tabular-nums text-muted">
              {h()!.done.toLocaleString()}
              <Show when={h()!.total > 0}> / {h()!.total.toLocaleString()}</Show>
            </span>
            <Show when={h()!.state !== "done"}>
              <button
                class="ml-2 grid size-6 place-items-center rounded-full text-muted hover:bg-hover"
                title={h()!.state === "paused" ? "Resume" : "Pause"}
                onClick={() => p.onPause(h()!.state !== "paused")}
              >
                <Icon name={h()!.state === "paused" ? "play" : "pause"} size={16} />
              </button>
            </Show>
          </div>
          <Show when={h()!.state !== "done"}>
            <div class="h-1 overflow-hidden rounded-full bg-surface">
              <div class="h-full rounded-full bg-primary transition-[width] duration-300" style={{ width: `${(frac() ?? 0) * 100}%` }} />
            </div>
            <Show when={h()!.state === "running"}>
              <div class="mt-1 text-[11px] text-muted">{formatEta(h()!.etaSecs)}</div>
            </Show>
          </Show>
        </div>
      </Show>
      <Row title="Mail on this PC" stack>
        <Segmented<SyncWindow>
          label="Mail on this PC"
          value={p.account.syncWindow}
          options={WINDOWS.map((w) => [w.id, w.short] as [SyncWindow, string])}
          onChange={p.onWindow}
        />
      </Row>
      <Row title="Notifications" stack>
        <Segmented<NotificationMode>
          label="Notifications"
          value={p.account.notify}
          options={[
            ["all", "All new mail"],
            ["otp", "Codes only"],
            ["off", "Off"],
          ]}
          onChange={p.onNotify}
        />
      </Row>
    </div>
  );
}

function Segmented<T extends string>(p: { value: T; options: [T, string][]; onChange(v: T): void; label: string }) {
  return (
    <div class="flex rounded-full border border-outline p-0.5" role="radiogroup" aria-label={p.label}>
      <For each={p.options}>
        {([v, text]) => (
          <button
            role="radio"
            aria-checked={p.value === v}
            class={`flex h-8 flex-1 items-center justify-center gap-1 whitespace-nowrap rounded-full px-2 text-xs font-medium transition-colors ${
              p.value === v ? "bg-secondary-container text-on-secondary-container" : "text-muted hover:bg-hover"
            }`}
            onClick={() => p.onChange(v)}
          >
            <Show when={p.value === v}>
              <svg width="14" height="14" viewBox="0 -960 960 960" fill="currentColor" aria-hidden="true">
                <path d="M382-240 154-468l57-57 171 171 367-367 57 57-424 424Z" />
              </svg>
            </Show>
            {text}
          </button>
        )}
      </For>
    </div>
  );
}

function Toggle(p: { checked: boolean; onChange(v: boolean): void; label: string }) {
  return (
    <button
      role="switch"
      aria-checked={p.checked}
      aria-label={p.label}
      class={`relative h-8 w-[52px] shrink-0 rounded-full border-2 transition-colors ${
        p.checked ? "border-primary bg-primary" : "border-outline bg-surface-container"
      }`}
      onClick={() => p.onChange(!p.checked)}
    >
      <span
        class={`absolute top-1/2 -translate-y-1/2 rounded-full transition-all duration-200 ${
          p.checked ? "left-[22px] size-6 bg-on-primary" : "left-1.5 size-4 bg-outline"
        }`}
      />
    </button>
  );
}

function Row(p: { title: string; desc?: string; children: JSX.Element; stack?: boolean }) {
  return (
    <div class={`flex gap-4 py-3 ${p.stack ? "flex-col" : "items-center"}`}>
      <div class="min-w-0 flex-1">
        <div class="text-sm text-text">{p.title}</div>
        <Show when={p.desc}>
          <div class="mt-0.5 text-xs text-muted">{p.desc}</div>
        </Show>
      </div>
      {p.children}
    </div>
  );
}

const Section = (p: { title: string; children: JSX.Element }) => (
  <section class="mb-2">
    <h3 class="mb-1 mt-4 text-xs font-medium uppercase tracking-wide text-primary">{p.title}</h3>
    {p.children}
  </section>
);

export default function SettingsPanel(p: Props) {
  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        p.onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    onCleanup(() => window.removeEventListener("keydown", onKey, true));
  });

  return (
    <div class="fixed inset-0 z-50" role="dialog" aria-modal="true" aria-label="Settings">
      <div class="absolute inset-0 bg-black/30 animate-fade-in" onClick={p.onClose} />
      <aside class="absolute inset-y-0 right-0 flex w-[420px] max-w-full flex-col bg-surface-high shadow-2xl animate-sheet-in">
        <header class="flex h-16 shrink-0 items-center gap-2 px-4">
          <Icon name="settings" size={22} class="text-muted" />
          <h2 class="flex-1 font-display text-xl">Settings</h2>
          <button class="grid size-10 place-items-center rounded-full text-muted hover:bg-hover" onClick={p.onClose} title="Close (Esc)">
            <Icon name="close" />
          </button>
        </header>
        <div class="flex-1 overflow-y-auto px-6 pb-8">
          <Section title="Accounts">
            <For each={p.accounts}>
              {(a) => (
                <AccountCard
                  account={a}
                  onRemove={() => p.onRemoveAccount(a.id)}
                  onWindow={(w) => p.onSyncWindow(a.id, w)}
                  onNotify={(m) => p.onAccountNotify(a.id, m)}
                  onPause={(x) => p.onPauseHistory(a.id, x)}
                />
              )}
            </For>
            <button class="flex h-10 items-center gap-2 rounded-full px-4 text-sm font-medium text-primary hover:bg-primary/10" onClick={p.onAddAccount}>
              <Icon name="add" size={20} /> Add another account
            </button>
          </Section>

          <Section title="Storage">
            <Row title="Full messages kept offline" desc="Older messages download when you open them." stack>
              <Segmented<Prefs["offline_days"]>
                label="Offline messages"
                value={p.prefs.offline_days}
                options={[
                  ["30", "30 days"],
                  ["90", "90 days"],
                  ["365", "1 year"],
                ]}
                onChange={(v) => p.onPref("offline_days", v)}
              />
            </Row>
            <Row title="Storage budget" desc={`Using ${formatSize(p.status.dbBytes)}. Offline copies stop when the budget is reached.`} stack>
              <Segmented<Prefs["storage_budget_mb"]>
                label="Storage budget"
                value={p.prefs.storage_budget_mb}
                options={[
                  ["512", "512 MB"],
                  ["1024", "1 GB"],
                  ["2048", "2 GB"],
                  ["5120", "5 GB"],
                ]}
                onChange={(v) => p.onPref("storage_budget_mb", v)}
              />
            </Row>
          </Section>

          <Section title="General">
            <Row title="Start with Windows" desc="Opens minimized to the tray when you sign in to Windows.">
              <Toggle label="Start with Windows" checked={p.status.autostart} onChange={p.onAutostart} />
            </Row>
            <Row title="When I close the window" stack>
              <Segmented
                label="Close behavior"
                value={p.status.destroyOnClose ? "quit" : "tray"}
                options={[
                  ["tray", "Keep running in tray"],
                  ["quit", "Quit Pulse Mail"],
                ]}
                onChange={(v) => p.onCloseBehavior(v === "quit")}
              />
            </Row>
          </Section>

          <Section title="Appearance">
            <Row title="Theme" stack>
              <Segmented<ThemePref>
                label="Theme"
                value={theme.pref()}
                options={[
                  ["system", "System"],
                  ["light", "Light"],
                  ["dark", "Dark"],
                ]}
                onChange={(v) => theme.set(v)}
              />
            </Row>
            <Row title="Density" desc="How tall each conversation row is." stack>
              <Segmented<Density>
                label="Density"
                value={prefs.density()}
                options={[
                  ["comfortable", "Comfortable"],
                  ["default", "Default"],
                  ["compact", "Compact"],
                ]}
                onChange={(v) => prefs.setDensity(v)}
              />
            </Row>
          </Section>

          <Section title="Reading mail">
            <Row title="Reading pane" desc="Where a conversation opens. Small windows always use the full page." stack>
              <Segmented<Pane>
                label="Reading pane"
                value={prefs.pane()}
                options={[
                  ["right", "Right of list"],
                  ["bottom", "Below list"],
                  ["full", "Full page"],
                ]}
                onChange={(v) => prefs.setPane(v)}
              />
            </Row>
            <Row title="Images from the web" desc="Remote images can tell senders you opened a mail. Pulse Mail fetches them for you, never loads tracking pixels, and the message itself stays offline." stack>
              <Segmented<Prefs["images"]>
                label="Images"
                value={p.prefs.images}
                options={[
                  ["ask", "Ask before showing"],
                  ["always", "Always show"],
                ]}
                onChange={(v) => p.onPref("images", v)}
              />
            </Row>
            <Row title="Email colors in dark theme" stack>
              <Segmented<Prefs["email_dark"]>
                label="Email colors in dark theme"
                value={p.prefs.email_dark}
                options={[
                  ["smart", "Dark (inverted)"],
                  ["original", "Original colors"],
                ]}
                onChange={(v) => p.onPref("email_dark", v)}
              />
            </Row>
          </Section>

          <Section title="Verification codes">
            <Row title="Copy new codes automatically" desc="When a code arrives, put it on the clipboard right away.">
              <Toggle label="Copy new codes automatically" checked={p.prefs.otp_autocopy === "1"} onChange={(v) => p.onPref("otp_autocopy", v ? "1" : "0")} />
            </Row>
            <Row title="Clear copied codes from the clipboard" desc="Only if the code is still what's on the clipboard." stack>
              <Segmented<Prefs["clipboard_clear_secs"]>
                label="Clear copied codes"
                value={p.prefs.clipboard_clear_secs}
                options={[
                  ["0", "Never"],
                  ["30", "30 s"],
                  ["60", "1 min"],
                  ["120", "2 min"],
                ]}
                onChange={(v) => p.onPref("clipboard_clear_secs", v)}
              />
            </Row>
            <Row title="Hide codes on the lock screen" desc="While your PC is locked, toasts say a code arrived without showing it.">
              <Toggle label="Hide codes on the lock screen" checked={p.prefs.otp_lockscreen === "hide"} onChange={(v) => p.onPref("otp_lockscreen", v ? "hide" : "show")} />
            </Row>
          </Section>

          <Section title="Notifications">
            <Row title="Desktop notifications (all accounts)" desc="Sets every account at once; change one account above. Codes always get a Copy button." stack>
              <Segmented<NotificationMode>
                label="Notifications"
                value={p.status.notifications}
                options={[
                  ["all", "All new mail"],
                  ["otp", "Codes only"],
                  ["off", "Off"],
                ]}
                onChange={p.onNotifications}
              />
            </Row>
          </Section>

          <Section title="About">
            <div class="space-y-1 py-2 text-xs text-muted">
              <div>Pulse Mail {p.status.version}</div>
              <div class="selectable break-all">Local data: {p.status.dataDir}</div>
              <div>Your mail stays on this PC. Sign-in tokens live in Windows Credential Manager.</div>
            </div>
          </Section>
        </div>
      </aside>
    </div>
  );
}
