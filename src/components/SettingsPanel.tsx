// Settings side sheet. Every control applies immediately; nothing to "save".
import { createSignal, For, onCleanup, onMount, Show, type JSX } from "solid-js";
import HistoryProgress from "./HistoryProgress";
import { api } from "../lib/api";
import Icon from "./Icon";
import { theme, type ThemePref } from "../lib/theme";
import { prefs, type Density } from "../lib/prefs";
import Avatar from "./Avatar";
import { snack } from "./Snackbar";
import { phaseLabel, WINDOWS } from "../lib/accounts";
import { formatSize } from "../lib/format";
import type { Account, AccountSettings, AccountStorage, AppStatus, AttachMode, NotificationMode, PrefKey, Prefs, SyncWindow } from "../lib/types";

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
  onAccountSettings(accountId: number, settings: AccountSettings): void;
}

export const ACCOUNT_COLORS = ["#c2410c", "#b91c1c", "#be185d", "#7c3aed", "#1d4ed8", "#0e7490", "#047857", "#4d7c0f"];

export function accountName(a: Account) {
  return a.displayName || a.email;
}

/** "off" plus every half hour of the day, as minutes since midnight. */
const QUIET_OPTIONS = ["off", ...Array.from({ length: 48 }, (_, i) => String(i * 30))];

function quietLabel(v: string): string {
  if (v === "off") return "Off";
  const m = Number(v);
  const h24 = Math.floor(m / 60);
  const h = h24 % 12 === 0 ? 12 : h24 % 12;
  return `${h}:${String(m % 60).padStart(2, "0")} ${h24 >= 12 ? "PM" : "AM"}`;
}

function QuietSelect(p: { label: string; value: string; onChange(v: string): void }) {
  return (
    <select
      class="h-9 rounded-lg border border-outline bg-surface px-2 text-sm text-text"
      aria-label={p.label}
      value={p.value}
      onChange={(e) => p.onChange(e.currentTarget.value)}
    >
      <For each={QUIET_OPTIONS}>
        {(v) => (
          <option value={v} selected={v === p.value}>
            {quietLabel(v)}
          </option>
        )}
      </For>
    </select>
  );
}

function AccountRow(p: { account: Account; onOpen(): void }) {
  const h = () => p.account.sync.history;
  return (
    <button class="mb-2 flex w-full items-center gap-3 rounded-2xl border border-divider p-3 text-left hover:bg-hover" onClick={p.onOpen}>
      <Avatar name={accountName(p.account)} email={p.account.email} size={36} color={p.account.color} picture={p.account.avatarData} />
      <div class="min-w-0 flex-1">
        <div class="truncate text-sm font-medium text-text">{accountName(p.account)}</div>
        <div class={`truncate text-xs ${p.account.sync.error || h()?.error ? "text-danger" : "text-muted"}`}>
          <Show when={p.account.displayName}>{p.account.email} · </Show>
          {p.account.sync.error ??
            (h() && h()!.state !== "done" && h()!.state !== "setup"
              ? phaseLabel(h()!)
              : p.account.sync.imapConnected
                ? "Live"
                : p.account.sync.running
                  ? "Connected"
                  : "Not syncing")}
        </div>
      </div>
      <Icon name="back" size={18} class="rotate-180 text-muted" />
    </button>
  );
}

function AccountPage(p: {
  account: Account;
  onBack(): void;
  onRemove(): void;
  onWindow(w: SyncWindow): void;
  onNotify(m: NotificationMode): void;
  onPause(pause: boolean): void;
  onRetry(): void;
  onSettings(s: AccountSettings): void;
}) {
  const h = () => p.account.sync.history;
  const [storage, setStorage] = createSignal<AccountStorage | null>(null);
  const [reconnecting, setReconnecting] = createSignal(false);
  const [profileError, setProfileError] = createSignal<string | null>(null);
  const reconnect = async () => {
    if (reconnecting()) return;
    setReconnecting(true);
    setProfileError(null);
    try {
      await api.signIn(p.account.id);
      snack("Google account reconnected.");
    } catch (e) {
      setProfileError(String(e));
    } finally {
      setReconnecting(false);
    }
  };
  onMount(() => void api.accountStorage(p.account.id).then(setStorage, () => {}));
  const saveText = (key: "displayName" | "signature", v: string) => {
    if (v !== p.account[key]) p.onSettings({ [key]: v });
  };
  return (
    <div>
      <button class="mb-2 mt-2 flex h-8 items-center gap-1 rounded-full pl-2 pr-3 text-sm text-muted hover:bg-hover" onClick={p.onBack}>
        <Icon name="back" size={18} /> All settings
      </button>
      <div class="mb-4 flex items-center gap-3">
        <Avatar name={accountName(p.account)} email={p.account.email} size={44} color={p.account.color} picture={p.account.avatarData} />
        <div class="min-w-0 flex-1">
          <div class="truncate font-display text-lg text-text">{accountName(p.account)}</div>
          <Show when={p.account.displayName}>
            <div class="selectable truncate text-xs text-muted">{p.account.email}</div>
          </Show>
        </div>
      </div>
      <Show when={h() && h()!.state !== "setup"}>
        <HistoryProgress history={h()!} onPause={p.onPause} onRetry={p.onRetry} />
      </Show>
      <Section title="This account">
        <Row title="Google profile" desc="Update your Google name and photo. Your saved mail and settings stay in place." stack>
          <button class="h-9 self-start rounded-full px-4 text-sm font-medium text-primary hover:bg-primary/10 disabled:opacity-50" disabled={reconnecting()} onClick={() => void reconnect()}>
            {reconnecting() ? "Waiting for Google…" : "Reconnect Google"}
          </button>
          <Show when={profileError()}>{(error) => <p class="text-xs text-danger" role="alert">{error()}</p>}</Show>
        </Row>
        <Row title="Name" desc="Shown in the account menu and as the sender name." stack>
          <input
            class="h-9 w-full rounded-lg border border-outline bg-surface px-3 text-sm text-text"
            dir="auto"
            maxLength={80}
            placeholder={p.account.email}
            value={p.account.displayName}
            onBlur={(e) => saveText("displayName", e.currentTarget.value.trim())}
          />
        </Row>
        <Row title="Color" stack>
          <div class="flex flex-wrap gap-2" role="radiogroup" aria-label="Account color">
            <button
              role="radio"
              aria-checked={!p.account.color}
              class={`h-7 rounded-full border px-3 text-xs ${!p.account.color ? "border-primary text-primary" : "border-outline text-muted"}`}
              onClick={() => p.onSettings({ color: "" })}
            >
              Auto
            </button>
            <For each={ACCOUNT_COLORS}>
              {(c) => (
                <button
                  role="radio"
                  aria-checked={p.account.color === c}
                  aria-label={c}
                  class={`size-7 rounded-full ${p.account.color === c ? "ring-2 ring-primary ring-offset-2 ring-offset-surface-high" : ""}`}
                  style={{ background: c }}
                  onClick={() => p.onSettings({ color: c })}
                />
              )}
            </For>
          </div>
        </Row>
        <Row title="Notifications" stack>
          <Segmented<NotificationMode>
            label="Notifications"
            value={p.account.notify}
            options={[
              ["all", "All new mail"],
              ["primary", "Primary + codes"],
              ["otp", "Codes only"],
              ["off", "Off"],
            ]}
            onChange={p.onNotify}
          />
        </Row>
        <Row title="Inbox tabs" desc="Primary, Promotions, Social, Updates and Forums, as sorted by Gmail.">
          <Toggle
            label="Inbox tabs"
            checked={p.account.categoryTabs !== false}
            onChange={(v) => p.onSettings({ categoryTabs: v })}
          />
        </Row>
        <Row title="Signature" desc="Added to new messages, replies and forwards from this account." stack>
          <textarea
            class="min-h-20 w-full rounded-lg border border-outline bg-surface px-3 py-2 text-sm text-text"
            dir="auto"
            maxLength={2000}
            value={p.account.signature}
            onBlur={(e) => saveText("signature", e.currentTarget.value)}
          />
        </Row>
      </Section>
      <Section title="Offline mail">
        <Row title="Download to this PC" desc="Full messages in this range stay on this PC for offline reading and search. Older mail downloads when you open it." stack>
          <Segmented<SyncWindow>
            label="Download to this PC"
            value={p.account.syncWindow}
            options={WINDOWS.map((w) => [w.id, w.short] as [SyncWindow, string])}
            onChange={p.onWindow}
          />
        </Row>
        <Row title="Attachments" desc="Saved files count against the storage budget. Others download when you open them." stack>
          <Segmented<AttachMode>
            label="Attachments"
            value={p.account.attachMode}
            options={[
              ["none", "On demand"],
              ["limit", "Small files"],
              ["all", "All files"],
            ]}
            onChange={(v) => p.onSettings({ attachMode: v })}
          />
          <Show when={p.account.attachMode === "limit"}>
            <Segmented<string>
              label="Largest file to keep"
              value={String(p.account.attachMaxMb)}
              options={[
                ["2", "Up to 2 MB"],
                ["10", "Up to 10 MB"],
                ["25", "Up to 25 MB"],
              ]}
              onChange={(v) => p.onSettings({ attachMaxMb: Number(v) })}
            />
          </Show>
        </Row>
        <Row title="Storage used" stack>
          <div class="text-xs text-muted">
            <Show when={storage()} fallback="…">
              {storage()!.messages.toLocaleString("en-US")} messages · {formatSize(storage()!.dbBytes)} of mail · {formatSize(storage()!.filesBytes)} of attachments
            </Show>
          </div>
        </Row>
      </Section>
      <Section title="Remove">
        <Row title="Remove this account" desc="Signs out and deletes its mail and attachments from this PC. Nothing is deleted in Gmail.">
          <button class="h-8 shrink-0 rounded-full px-3 text-xs font-medium text-danger hover:bg-danger/10" onClick={p.onRemove}>
            Remove
          </button>
        </Row>
      </Section>
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

function ReadingLayout() {
  const options = [
    { value: "right", title: "Side pane", desc: "Keep the list visible and read on the right.", icon: "splitRight" },
    { value: "full", title: "Full view", desc: "Replace the list with the email, like Gmail. Back returns to the list.", icon: "noSplit" },
    { value: "bottom", title: "Below list", desc: "Keep the list above the email.", icon: "splitBottom" },
  ] as const;
  return (
    <fieldset class="mb-2 mt-4">
      <legend class="text-xs font-medium uppercase tracking-wide text-primary">Open emails</legend>
      <p id="reading-layout-help" class="mb-2 mt-2 text-xs text-muted">
        Applies immediately and is remembered. Split views use full view when the window is too small.
      </p>
      <div aria-describedby="reading-layout-help">
        <For each={options}>
          {(option) => (
            <label class={`flex cursor-pointer items-center gap-3 rounded-xl px-3 py-3 transition-colors ${prefs.pane() === option.value ? "bg-secondary-container text-on-secondary-container" : "text-text hover:bg-hover"}`}>
              <Icon name={option.icon} size={24} class="shrink-0" />
              <span class="min-w-0 flex-1">
                <span class="block text-sm font-medium">{option.title}</span>
                <span class={`mt-0.5 block text-xs ${prefs.pane() === option.value ? "text-on-secondary-container" : "text-muted"}`}>{option.desc}</span>
              </span>
              <input
                type="radio"
                name="reading-layout"
                value={option.value}
                aria-label={option.title}
                checked={prefs.pane() === option.value}
                onChange={() => prefs.setPane(option.value)}
                aria-describedby="reading-layout-help"
                class="size-4 shrink-0 accent-primary"
              />
            </label>
          )}
        </For>
      </div>
    </fieldset>
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
  const [page, setPage] = createSignal<number | null>(null);
  const pageAccount = () => p.accounts.find((a) => a.id === page());
  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        if (page() != null) setPage(null);
        else p.onClose();
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
          <Show when={pageAccount()}>
            {(a) => (
              <AccountPage
                account={a()}
                onBack={() => setPage(null)}
                onRemove={() => p.onRemoveAccount(a().id)}
                onWindow={(w) => p.onSyncWindow(a().id, w)}
                onNotify={(m) => p.onAccountNotify(a().id, m)}
                onPause={(x) => p.onPauseHistory(a().id, x)}
                onRetry={() => p.onPauseHistory(a().id, false)}
                onSettings={(s) => p.onAccountSettings(a().id, s)}
              />
            )}
          </Show>
          <Show when={!pageAccount()}>
          <ReadingLayout />
          <Section title="Accounts">
            <For each={p.accounts}>{(a) => <AccountRow account={a} onOpen={() => setPage(a.id)} />}</For>
            <button class="flex h-10 items-center gap-2 rounded-full px-4 text-sm font-medium text-primary hover:bg-primary/10" onClick={p.onAddAccount}>
              <Icon name="add" size={20} /> Add another account
            </button>
          </Section>

          <Section title="Storage">
            <Row title="Storage budget" desc={`For all accounts: ${formatSize(p.status.dbBytes)} of mail, plus saved attachments (see each account). Downloads pause when the budget is reached.`} stack>
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

          <Section title="Sending mail">
            <Row
              title="Undo Send cancellation window"
              desc="Hold outgoing messages before transmitting to Google servers so you can undo mistakes."
              stack
            >
              <Segmented<Prefs["undo_send_secs"]>
                label="Undo Send cancellation window"
                value={p.prefs.undo_send_secs ?? "5"}
                options={[
                  ["0", "Off"],
                  ["5", "5 s"],
                  ["10", "10 s"],
                  ["20", "20 s"],
                  ["30", "30 s"],
                ]}
                onChange={(v) => p.onPref("undo_send_secs", v)}
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
                  ["primary", "Primary + codes"],
                  ["otp", "Codes only"],
                  ["off", "Off"],
                ]}
                onChange={p.onNotifications}
              />
            </Row>
            <Row title="Quiet hours" desc="Pause notification toasts between two times (they can cross midnight). Mail keeps syncing; only the toasts wait." stack>
              <div class="flex flex-wrap items-center gap-2">
                <QuietSelect label="Quiet from" value={p.prefs.quiet_start} onChange={(v) => p.onPref("quiet_start", v)} />
                <span class="text-xs text-muted">to</span>
                <QuietSelect label="Quiet until" value={p.prefs.quiet_end} onChange={(v) => p.onPref("quiet_end", v)} />
              </div>
            </Row>
            <Row title="Codes during quiet hours" desc="Verification codes are time-sensitive and can still toast during quiet hours.">
              <Toggle
                label="Codes during quiet hours"
                checked={p.prefs.quiet_allow_otp === "1"}
                onChange={(v) => p.onPref("quiet_allow_otp", v ? "1" : "0")}
              />
            </Row>
          </Section>

          <Section title="About">
            <div class="space-y-1 py-2 text-xs text-muted">
              <div>Pulse Mail {p.status.version}</div>
              <div class="selectable break-all">Local data: {p.status.dataDir}</div>
              <div>Your mail stays on this PC. Sign-in tokens live in Windows Credential Manager.</div>
            </div>
            <Row title="Logs" desc="Rolling local log files, kept for 7 days. Never message contents or tokens." stack>
              <button
                class="h-9 self-start rounded-full px-4 text-sm font-medium text-primary hover:bg-primary/10"
                onClick={() => void api.openLogsDir().catch((e: Error) => snack(e.message))}
              >
                Open logs folder
              </button>
            </Row>
          </Section>
          </Show>
        </div>
      </aside>
    </div>
  );
}
