// Settings side sheet. Every control applies immediately; nothing to "save".
import { For, onCleanup, onMount, Show, type JSX } from "solid-js";
import Icon from "./Icon";
import { theme, type ThemePref } from "../lib/theme";
import { prefs, type Density } from "../lib/prefs";
import type { AppStatus, NotificationMode } from "../lib/types";

interface Props {
  status: AppStatus;
  onClose(): void;
  onAutostart(enabled: boolean): void;
  onCloseBehavior(quit: boolean): void;
  onNotifications(mode: NotificationMode): void;
}

function Segmented<T extends string>(p: { value: T; options: [T, string][]; onChange(v: T): void; label: string }) {
  return (
    <div class="flex rounded-full border border-outline p-0.5" role="radiogroup" aria-label={p.label}>
      <For each={p.options}>
        {([v, text]) => (
          <button
            role="radio"
            aria-checked={p.value === v}
            class={`flex h-8 flex-1 items-center justify-center gap-1 rounded-full px-3 text-xs font-medium transition-colors ${
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

function Switch(p: { checked: boolean; onChange(v: boolean): void; label: string }) {
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
          <Section title="General">
            <Row title="Start with Windows" desc="Opens minimized to the tray when you sign in to Windows.">
              <Switch label="Start with Windows" checked={p.status.autostart} onChange={p.onAutostart} />
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

          <Section title="Notifications">
            <Row title="Desktop notifications" desc="Verification codes always get a Copy button on the toast." stack>
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
