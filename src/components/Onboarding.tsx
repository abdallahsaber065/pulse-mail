// Multi-step onboarding, for the first account and for "Add account":
// welcome → sign in → what to download → notifications → live download
// progress (can continue to the inbox while it runs) → done.
import { createEffect, createResource, createSignal, For, Match, on, Show, Switch, type JSX } from "solid-js";
import Icon, { type IconName } from "./Icon";
import { api } from "../lib/api";
import { compactCount, formatEta, formatRate, historyFraction, phaseLabel, WINDOWS } from "../lib/accounts";
import { formatSize } from "../lib/format";
import type { History, NotificationMode, PrefKey, Prefs, SyncWindow } from "../lib/types";

type Step = "welcome" | "signin" | "range" | "alerts" | "progress" | "done";
const STEPS: { id: Step; label: string }[] = [
  { id: "signin", label: "Sign in" },
  { id: "range", label: "Download" },
  { id: "alerts", label: "Notifications" },
  { id: "progress", label: "Sync" },
];

export interface OnboardingProps {
  mode: "first" | "add";
  /** Continue setting up an account that signed in but didn't finish. */
  resume?: { accountId: number; email: string };
  clientConfigured: boolean;
  clientConfigPath: string;
  onRecheck(): void;
  prefs: Prefs;
  onPref<K extends PrefKey>(key: K, value: Prefs[K]): void;
  /** Live history state of an account (from sync-status events). */
  history(accountId: number): History | null;
  onSignedIn(): void;
  onClose(): void;
}

function Choice(p: { selected: boolean; onSelect(): void; icon: IconName; title: string; desc?: JSX.Element; badge?: string }) {
  return (
    <button
      role="radio"
      aria-checked={p.selected}
      class={`flex w-full items-center gap-4 rounded-2xl border px-4 py-3 text-left transition-colors ${
        p.selected ? "border-primary bg-primary/10" : "border-outline hover:bg-hover"
      }`}
      onClick={p.onSelect}
    >
      <span class={`grid size-10 shrink-0 place-items-center rounded-full ${p.selected ? "bg-primary text-on-primary" : "bg-surface-container text-muted"}`}>
        <Icon name={p.icon} size={22} />
      </span>
      <span class="min-w-0 flex-1">
        <span class="flex items-center gap-2 text-sm font-medium text-text">
          {p.title}
          <Show when={p.badge}>
            <span class="rounded-full bg-secondary-container px-2 py-0.5 text-[11px] font-medium text-on-secondary-container">{p.badge}</span>
          </Show>
        </span>
        <Show when={p.desc}>
          <span class="mt-0.5 block text-xs text-muted">{p.desc}</span>
        </Show>
      </span>
      <span class={`grid size-5 shrink-0 place-items-center rounded-full border-2 ${p.selected ? "border-primary" : "border-outline"}`}>
        <Show when={p.selected}>
          <span class="size-2.5 rounded-full bg-primary" />
        </Show>
      </span>
    </button>
  );
}

function Pills<T extends string>(p: { value: T; options: [T, string][]; onChange(v: T): void; label: string }) {
  return (
    <div class="flex flex-wrap gap-2" role="radiogroup" aria-label={p.label}>
      <For each={p.options}>
        {([v, text]) => (
          <button
            role="radio"
            aria-checked={p.value === v}
            class={`h-8 rounded-lg border px-3 text-xs font-medium transition-colors ${
              p.value === v ? "border-transparent bg-secondary-container text-on-secondary-container" : "border-outline text-muted hover:bg-hover"
            }`}
            onClick={() => p.onChange(v)}
          >
            {text}
          </button>
        )}
      </For>
    </div>
  );
}

export default function Onboarding(p: OnboardingProps) {
  const [step, setStep] = createSignal<Step>(p.resume ? "range" : p.mode === "first" ? "welcome" : "signin");
  const [account, setAccount] = createSignal<{ accountId: number; email: string } | null>(p.resume ?? null);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [window, setWindow] = createSignal<SyncWindow>("90d");
  const [notify, setNotify] = createSignal<NotificationMode>("all");

  const [estimates] = createResource(
    () => (step() === "range" ? account()?.accountId : undefined),
    (id) => api.estimates(id).catch(() => []),
  );
  const estimate = (w: SyncWindow) => estimates()?.find((e) => e.window === w);

  const history = () => {
    const a = account();
    return a ? p.history(a.accountId) : null;
  };
  createEffect(
    on(history, (h) => {
      if (step() === "progress" && h?.state === "done") setStep("done");
    }),
  );

  const signIn = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await api.signIn();
      setAccount({ accountId: r.accountId, email: r.email });
      p.onSignedIn();
      setStep(r.needsSetup ? "range" : "done");
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const start = async () => {
    const a = account();
    if (!a) return;
    setBusy(true);
    setError(null);
    try {
      await api.finishOnboarding(a.accountId, window(), notify());
      setStep("progress");
      p.onSignedIn();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const togglePause = async () => {
    const a = account();
    const h = history();
    if (!a || !h) return;
    try {
      if (h.state === "paused") await api.resumeHistory(a.accountId);
      else await api.pauseHistory(a.accountId);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const stepIndex = () => STEPS.findIndex((s) => s.id === step());
  const closable = () => p.mode === "add" || step() === "progress" || step() === "done";

  const Primary = (b: { onClick(): void; children: JSX.Element; disabled?: boolean; icon?: IconName }) => (
    <button
      class="flex h-10 items-center gap-2 rounded-full bg-primary px-6 text-sm font-medium text-on-primary transition-shadow hover:shadow disabled:opacity-60"
      onClick={b.onClick}
      disabled={b.disabled}
    >
      {b.children}
      <Show when={b.icon}>
        <Icon name={b.icon!} size={18} />
      </Show>
    </button>
  );
  const Secondary = (b: { onClick(): void; children: JSX.Element; icon?: IconName }) => (
    <button class="flex h-10 items-center gap-2 rounded-full px-4 text-sm font-medium text-primary hover:bg-primary/10" onClick={b.onClick}>
      <Show when={b.icon}>
        <Icon name={b.icon!} size={18} />
      </Show>
      {b.children}
    </button>
  );

  return (
    <div class="fixed inset-0 z-[60] grid place-items-center overflow-y-auto bg-surface-dim/95 p-6 animate-fade-in" role="dialog" aria-modal="true" aria-label="Set up an account">
      <div class="relative w-full max-w-[600px] rounded-[28px] bg-surface-high p-8 shadow-2xl">
        <Show when={closable()}>
          <button
            class="absolute right-4 top-4 grid size-10 place-items-center rounded-full text-muted hover:bg-hover"
            onClick={p.onClose}
            title={step() === "progress" ? "Continue to inbox" : "Close"}
          >
            <Icon name="close" />
          </button>
        </Show>

        <Show when={stepIndex() >= 0}>
          <ol class="mb-6 flex items-center gap-2 pr-10 text-xs" aria-label="Steps">
            <For each={STEPS}>
              {(s, i) => (
                <li class="flex items-center gap-2">
                  <span
                    class={`grid size-6 place-items-center rounded-full text-[11px] font-medium ${
                      i() < stepIndex() ? "bg-primary text-on-primary" : i() === stepIndex() ? "bg-secondary-container text-on-secondary-container" : "bg-surface-container text-faint"
                    }`}
                  >
                    {i() < stepIndex() ? <Icon name="check" size={14} /> : i() + 1}
                  </span>
                  <span class={i() === stepIndex() ? "font-medium text-text" : "text-muted"}>{s.label}</span>
                  <Show when={i() < STEPS.length - 1}>
                    <span class="mx-1 h-px w-6 bg-divider" />
                  </Show>
                </li>
              )}
            </For>
          </ol>
        </Show>

        <Switch>
          <Match when={step() === "welcome"}>
            <div class="py-4 text-center">
              <div class="mx-auto mb-5 grid size-16 place-items-center rounded-[20px] bg-primary text-on-primary shadow-lg">
                <Icon name="mail" size={34} />
              </div>
              <h1 class="mb-2 font-display text-[28px] leading-9 text-text">Welcome to Pulse Mail</h1>
              <p class="mx-auto mb-8 max-w-md text-sm leading-6 text-muted">
                Gmail on your desktop, live. New mail and verification codes appear the moment they arrive, and your mail stays
                readable offline.
              </p>
              <div class="mb-8 grid grid-cols-3 gap-3 text-left">
                <For
                  each={
                    [
                      ["bolt", "Instant", "Push, not polling"],
                      ["key", "Codes", "One-click copy"],
                      ["shield", "Private", "Images blocked, mail stays on this PC"],
                    ] as [IconName, string, string][]
                  }
                >
                  {([icon, title, desc]) => (
                    <div class="rounded-2xl bg-surface-container p-3">
                      <Icon name={icon} size={20} class="mb-2 text-primary" />
                      <div class="text-sm font-medium text-text">{title}</div>
                      <div class="text-xs text-muted">{desc}</div>
                    </div>
                  )}
                </For>
              </div>
              <div class="flex justify-center">
                <Primary onClick={() => setStep("signin")} icon="arrowForward">
                  Get started
                </Primary>
              </div>
            </div>
          </Match>

          <Match when={step() === "signin"}>
            <h1 class="mb-2 font-display text-2xl text-text">{p.mode === "add" ? "Add a Google account" : "Sign in with Google"}</h1>
            <Show
              when={p.clientConfigured}
              fallback={
                <>
                  <p class="mb-4 text-sm leading-6 text-muted">
                    Pulse Mail signs in with your own Google Cloud OAuth client. Create a Desktop-app client (see
                    <span class="font-medium text-text"> docs/SETUP-GOOGLE-OAUTH.md</span>), download its JSON, and save it as:
                  </p>
                  <code class="selectable mb-6 block break-all rounded-lg bg-surface-container px-3 py-2 font-mono text-xs text-text">{p.clientConfigPath}</code>
                  <Primary onClick={p.onRecheck}>I've saved it</Primary>
                </>
              }
            >
              <p class="mb-6 text-sm leading-6 text-muted">
                Your browser opens Google's sign-in page. Pulse Mail never sees your password; the sign-in is kept in Windows
                Credential Manager on this PC only.
              </p>
              <ul class="mb-8 space-y-2 text-sm text-text">
                <li class="flex gap-3">
                  <Icon name="checkCircle" size={20} class="text-otp" /> Read, organize and send mail from this PC
                </li>
                <li class="flex gap-3">
                  <Icon name="checkCircle" size={20} class="text-otp" /> Live updates over Gmail's secure connection
                </li>
                <li class="flex gap-3">
                  <Icon name="checkCircle" size={20} class="text-otp" /> Remove the account anytime in Settings
                </li>
              </ul>
              <div class="flex items-center gap-3">
                <Primary onClick={() => void signIn()} disabled={busy()}>
                  {busy() ? "Finish signing in in your browser…" : "Sign in with Google"}
                </Primary>
                <Show when={busy()}>
                  <span class="text-xs text-muted">Waiting for Google…</span>
                </Show>
              </div>
            </Show>
          </Match>

          <Match when={step() === "range"}>
            <h1 class="mb-1 font-display text-2xl text-text">What should be on this PC?</h1>
            <p class="mb-5 text-sm text-muted">
              Signed in as <span class="font-medium text-text">{account()?.email}</span>. Your inbox is already loading. Choose how much
              history to download for offline reading and search; older mail still opens on demand.
            </p>
            <div class="mb-5 space-y-2" role="radiogroup" aria-label="How much mail to download">
              <For each={WINDOWS}>
                {(w) => (
                  <Choice
                    selected={window() === w.id}
                    onSelect={() => setWindow(w.id)}
                    icon={w.id === "all" ? "allInbox" : "history"}
                    title={w.label}
                    badge={w.id === "90d" ? "Recommended" : undefined}
                    desc={
                      <Show when={!estimates.loading} fallback="Estimating…">
                        <Show when={estimate(w.id)} fallback="Size unknown">
                          {(e) => (
                            <>
                              About {compactCount(e().messages)} messages · ~{formatSize(e().bytes)}
                            </>
                          )}
                        </Show>
                      </Show>
                    }
                  />
                )}
              </For>
            </div>
            <div class="mb-6 space-y-4 rounded-2xl bg-surface-container p-4">
              <div>
                <div class="mb-1 flex items-center gap-2 text-sm text-text">
                  <Icon name="download" size={18} class="text-muted" /> Full messages kept offline
                </div>
                <Pills<Prefs["offline_days"]>
                  label="Offline messages"
                  value={p.prefs.offline_days}
                  options={[
                    ["30", "30 days"],
                    ["90", "90 days"],
                    ["365", "1 year"],
                  ]}
                  onChange={(v) => p.onPref("offline_days", v)}
                />
              </div>
              <div>
                <div class="mb-1 flex items-center gap-2 text-sm text-text">
                  <Icon name="storage" size={18} class="text-muted" /> Storage budget
                </div>
                <Pills<Prefs["storage_budget_mb"]>
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
              </div>
              <div class="flex items-start gap-2 text-xs text-muted">
                <Icon name="attachment" size={18} class="shrink-0" />
                Attachments download when you open them. Files that can run programs are never opened automatically.
              </div>
            </div>
            <div class="flex justify-end">
              <Primary onClick={() => setStep("alerts")} icon="arrowForward">
                Next
              </Primary>
            </div>
          </Match>

          <Match when={step() === "alerts"}>
            <h1 class="mb-1 font-display text-2xl text-text">Notifications</h1>
            <p class="mb-5 text-sm text-muted">What should pop up on your desktop for {account()?.email}?</p>
            <div class="mb-6 space-y-2" role="radiogroup" aria-label="Notifications">
              <Choice selected={notify() === "all"} onSelect={() => setNotify("all")} icon="bell" title="Every new email" desc="A toast for each new message in your inbox." />
              <Choice
                selected={notify() === "otp"}
                onSelect={() => setNotify("otp")}
                icon="key"
                title="Only verification codes"
                desc="Sign-in codes with a one-click Copy button; everything else stays quiet."
              />
              <Choice selected={notify() === "off"} onSelect={() => setNotify("off")} icon="bellOff" title="Off" desc="Check Pulse Mail when you want to." />
            </div>
            <div class="flex items-center justify-between">
              <Secondary onClick={() => setStep("range")}>Back</Secondary>
              <Primary onClick={() => void start()} disabled={busy()} icon="cloudDownload">
                Start download
              </Primary>
            </div>
          </Match>

          <Match when={step() === "progress"}>
            {(() => {
              const h = () => history();
              const pct = () => historyFraction(h());
              return (
                <>
                  <h1 class="mb-1 font-display text-2xl text-text">Setting up {account()?.email}</h1>
                  <p class="mb-6 text-sm text-muted">
                    You can start reading now. The download continues in the background and picks up where it left off if you close Pulse
                    Mail.
                  </p>
                  <div class="mb-2 flex items-end justify-between">
                    <div>
                      <div class="text-sm font-medium text-text">{h() ? phaseLabel(h()!) : "Starting…"}</div>
                      <div class="text-xs text-muted tabular-nums">
                        <Show when={h() && h()!.total > 0} fallback="Counting messages…">
                          {h()!.done.toLocaleString()} of {h()!.total.toLocaleString()} {h()!.phase === "bodies" ? "offline copies" : "messages"}
                        </Show>
                      </div>
                    </div>
                    <div class="font-display text-3xl tabular-nums text-text">{pct() == null ? "…" : `${Math.floor(pct()! * 100)}%`}</div>
                  </div>
                  <div class="mb-3 h-2 overflow-hidden rounded-full bg-surface-container" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct() == null ? undefined : Math.floor(pct()! * 100)}>
                    <Show when={pct() != null} fallback={<div class="progress-indeterminate h-full bg-primary" />}>
                      <div class={`h-full rounded-full transition-[width] duration-500 ${h()?.state === "paused" ? "bg-outline" : "bg-primary"}`} style={{ width: `${Math.max(2, pct()! * 100)}%` }} />
                    </Show>
                  </div>
                  <div class="mb-6 flex items-center gap-3 text-xs text-muted tabular-nums">
                    <Show when={h()?.state === "paused"} fallback={<span>{formatEta(h()?.etaSecs)}</span>}>
                      <span>Paused. Resume anytime from here or Settings.</span>
                    </Show>
                    <Show when={h() && h()!.rate > 0 && h()!.state !== "paused"}>
                      <span class="size-1 rounded-full bg-outline" />
                      <span>{formatRate(h()!.rate)}</span>
                    </Show>
                  </div>
                  <div class="mb-6 grid grid-cols-2 gap-2 text-xs">
                    <For
                      each={
                        [
                          ["list", "1", "Message list", "Headers for search and browsing"],
                          ["bodies", "2", "Offline copies", `Full messages from the last ${p.prefs.offline_days} days`],
                        ] as const
                      }
                    >
                      {([phase, n, title, desc]) => {
                        const state = () => {
                          const ph = h()?.phase;
                          if (h()?.state === "done" || (phase === "list" && ph === "bodies")) return "done";
                          return ph === phase ? "active" : "todo";
                        };
                        return (
                          <div class={`rounded-xl border p-3 ${state() === "active" ? "border-primary" : "border-divider"}`}>
                            <div class="mb-0.5 flex items-center gap-2 font-medium text-text">
                              <span class={`grid size-5 place-items-center rounded-full text-[10px] ${state() === "done" ? "bg-otp text-white" : state() === "active" ? "bg-primary text-on-primary" : "bg-surface-container text-faint"}`}>
                                {state() === "done" ? <Icon name="check" size={12} /> : n}
                              </span>
                              {title}
                            </div>
                            <div class="text-muted">{desc}</div>
                          </div>
                        );
                      }}
                    </For>
                  </div>
                  <div class="flex items-center justify-between">
                    <Secondary onClick={() => void togglePause()} icon={h()?.state === "paused" ? "play" : "pause"}>
                      {h()?.state === "paused" ? "Resume" : "Pause"}
                    </Secondary>
                    <Primary onClick={p.onClose} icon="arrowForward">
                      Continue to inbox
                    </Primary>
                  </div>
                </>
              );
            })()}
          </Match>

          <Match when={step() === "done"}>
            <div class="py-4 text-center">
              <div class="mx-auto mb-5 grid size-16 place-items-center rounded-full bg-otp/15 text-otp">
                <Icon name="checkCircle" size={40} />
              </div>
              <h1 class="mb-2 font-display text-2xl text-text">{account()?.email} is ready</h1>
              <p class="mx-auto mb-8 max-w-sm text-sm text-muted">
                <Show when={history()?.done} fallback="Your mail is on this PC and new mail arrives live.">
                  {history()!.done.toLocaleString()} messages are on this PC, and new mail arrives live.
                </Show>
              </p>
              <div class="flex justify-center">
                <Primary onClick={p.onClose} icon="arrowForward">
                  Open inbox
                </Primary>
              </div>
            </div>
          </Match>
        </Switch>

        <Show when={error()}>
          <p class="mt-4 text-sm text-danger" role="alert">
            {error()}
          </p>
        </Show>
      </div>
    </div>
  );
}
