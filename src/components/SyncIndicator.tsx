// Transparency UI: a thin progress bar pinned to the top of the window, a
// status chip, and a details popover with the live activity log + retry.
import { createEffect, createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import Icon, { type IconName } from "./Icon";
import Popover from "./ui/Popover";
import { chipView, type ChipTone } from "../lib/sync-view";
import { relative } from "../lib/format";
import { windowLabel } from "../lib/accounts";
import HistoryProgress from "./HistoryProgress";
import type { Account, Activity, SyncStatus } from "../lib/types";

const TONE: Record<ChipTone, { dot: string; chip: string; icon: IconName }> = {
  live: { dot: "bg-otp pulse-dot", chip: "text-text", icon: "bolt" },
  ok: { dot: "bg-otp", chip: "text-text", icon: "cloudDone" },
  busy: { dot: "bg-primary", chip: "text-text", icon: "sync" },
  warn: { dot: "bg-star", chip: "text-text", icon: "cloudOff" },
  error: { dot: "bg-danger", chip: "bg-danger-container text-on-danger-container", icon: "error" },
  idle: { dot: "bg-outline", chip: "text-muted", icon: "info" },
};

const LEVEL_ICON: Record<Activity["level"], { icon: IconName; cls: string }> = {
  info: { icon: "info", cls: "text-muted" },
  success: { icon: "checkCircle", cls: "text-otp" },
  warn: { icon: "error", cls: "text-star" },
  error: { icon: "error", cls: "text-danger" },
};

/** Bar timing: sub-second work never flashes it. */
export const BAR_SHOW_DELAY_MS = 400;
export const BAR_MIN_VISIBLE_MS = 600;

export function TopProgress(p: { sync: SyncStatus | null; online: boolean }) {
  const v = createMemo(() => chipView(p.sync, p.online));
  const active = createMemo(() => v().bar !== undefined);
  // Appear only after BAR_SHOW_DELAY_MS of continuous work; once shown, stay
  // at least BAR_MIN_VISIBLE_MS so it never blinks.
  const [shown, setShown] = createSignal(false);
  let showTimer: number | undefined;
  let hideTimer: number | undefined;
  let shownAt = 0;
  createEffect(() => {
    if (active()) {
      clearTimeout(hideTimer);
      hideTimer = undefined;
      if (!shown() && showTimer === undefined) {
        showTimer = window.setTimeout(() => {
          showTimer = undefined;
          shownAt = Date.now();
          setShown(true);
        }, BAR_SHOW_DELAY_MS);
      }
    } else {
      clearTimeout(showTimer);
      showTimer = undefined;
      if (shown() && hideTimer === undefined) {
        hideTimer = window.setTimeout(
          () => {
            hideTimer = undefined;
            setShown(false);
          },
          Math.max(0, BAR_MIN_VISIBLE_MS - (Date.now() - shownAt)),
        );
      }
    }
  });
  onCleanup(() => {
    clearTimeout(showTimer);
    clearTimeout(hideTimer);
  });
  return (
    <div class="pointer-events-none fixed inset-x-0 top-0 z-[80] h-[3px] overflow-hidden" aria-hidden="true">
      <Show when={shown()}>
        <Show
          when={v().bar !== null}
          fallback={<div class={`progress-indeterminate h-full bg-primary ${v().barSubtle ? "opacity-50" : ""}`} />}
        >
          <div
            class={`h-full bg-primary transition-[width] duration-300 ease-out ${v().barSubtle ? "opacity-50" : ""}`}
            style={{ width: `${Math.max(4, (v().bar ?? 1) * 100)}%` }}
          />
        </Show>
      </Show>
    </div>
  );
}

export default function SyncIndicator(p: {
  sync: SyncStatus | null;
  online: boolean;
  activity: Activity[];
  accounts: Account[];
  onRetry(): void;
  onPauseHistory(accountId: number, pause: boolean): void;
}) {
  const [open, setOpen] = createSignal(false);
  const [now, setNow] = createSignal(Date.now());
  onMount(() => {
    const t = window.setInterval(() => {
      if (!document.body.classList.contains("win-hidden")) {
        setNow(Date.now());
      }
    }, 20_000);
    onCleanup(() => clearInterval(t));
  });
  const v = createMemo(() => chipView(p.sync, p.online, now()));
  const tone = () => TONE[v().tone];

  const Bar = (b: { label: string; done: number; total: number }) => (
    <div class="mb-3">
      <div class="mb-1 flex text-xs">
        <span class="flex-1 text-text">{b.label}</span>
        <span class="tabular-nums text-muted">
          {b.done}/{b.total}
        </span>
      </div>
      <div class="h-1 overflow-hidden rounded-full bg-surface-container">
        <div
          class="h-full rounded-full bg-primary transition-[width] duration-300"
          style={{ width: `${b.total ? (b.done / b.total) * 100 : 0}%` }}
        />
      </div>
    </div>
  );

  return (
    <div class="relative">
      <button
        class={`flex h-8 max-w-72 items-center gap-2 rounded-full px-3 text-xs transition-colors hover:bg-hover ${tone().chip}`}
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open()}
        aria-haspopup="dialog"
        title="Sync details"
      >
        <span class={`size-2 shrink-0 rounded-full ${tone().dot}`} />
        <span class="truncate tabular-nums">{v().text}</span>
        <Show when={v().retry}>
          <span
            role="button"
            class="font-medium text-primary hover:underline"
            onClick={(e) => {
              e.stopPropagation();
              p.onRetry();
            }}
          >
            Retry
          </span>
        </Show>
      </button>

      <Show when={open()}>
        <Popover label="Sync details" onClose={() => setOpen(false)} class="w-[360px] p-4">
          <div class="mb-3 flex items-center gap-3">
            <div class={`grid size-10 place-items-center rounded-full bg-surface-container ${v().tone === "error" ? "text-danger" : "text-primary"}`}>
              <Icon name={tone().icon} size={22} class={v().tone === "busy" ? "animate-spin-slow" : ""} />
            </div>
            <div class="min-w-0 flex-1">
              <div class="truncate text-sm font-medium">{v().text}</div>
              <div class="text-xs text-muted">
                {p.sync?.imapConnected
                  ? "Live push connected: new mail appears instantly"
                  : p.online
                    ? "Push not connected: checking every minute"
                    : "Waiting for the network"}
              </div>
            </div>
          </div>

          <Show when={p.sync?.progress}>{(pr) => <Bar {...pr()} />}</Show>

          {/* History download per account (H1). */}
          <For each={p.accounts.filter((a) => a.sync.history && a.sync.history.state !== "setup")}>
            {(a) => {
              const h = () => a.sync.history!;
              return (
                <div class="mb-3">
                  <Show when={p.accounts.length > 1}>
                    <div class="mb-1 truncate text-[11px] font-medium text-muted">{a.displayName || a.email}</div>
                  </Show>
                  <Show
                    when={h().state !== "done"}
                    fallback={<div class="rounded-xl bg-surface-container px-3 py-2 text-xs text-text">{windowLabel(h().window)} on this PC</div>}
                  >
                    <HistoryProgress compact history={h()} onPause={(x) => p.onPauseHistory(a.id, x)} onRetry={() => p.onPauseHistory(a.id, false)} />
                  </Show>
                </div>
              );
            }}
          </For>

          <div class="mb-2 flex items-center justify-between text-xs text-muted">
            <span>Last checked {p.sync?.lastSyncAt ? relative(p.sync.lastSyncAt, now()) : "never"}</span>
            <button
              class="flex h-8 items-center gap-1 rounded-full px-3 font-medium text-primary hover:bg-primary/10"
              onClick={() => p.onRetry()}
            >
              <Icon name="refresh" size={16} /> {v().retry ? "Retry now" : "Check now"}
            </button>
          </div>

          <div class="border-t border-divider pt-2">
            <div class="mb-1 flex items-center gap-1 text-xs font-medium uppercase tracking-wide text-muted">
              <Icon name="history" size={14} /> Activity
            </div>
            <Show when={p.activity.length > 0} fallback={<div class="py-3 text-xs text-muted">Nothing yet.</div>}>
              <ul class="max-h-64 overflow-y-auto pr-1">
                <For each={[...p.activity].reverse()}>
                  {(a) => (
                    <li class="flex gap-2 py-1.5 text-xs">
                      <Icon name={LEVEL_ICON[a.level].icon} size={16} class={`mt-px shrink-0 ${LEVEL_ICON[a.level].cls}`} />
                      <span class="selectable flex-1 text-text">
                        <Show when={p.accounts.length > 1 && a.account}>
                          <span class="text-muted">{a.account.split("@")[0]}: </span>
                        </Show>
                        {a.text}
                      </span>
                      <span class="shrink-0 text-faint" title={new Date(a.at).toLocaleString()}>
                        {new Date(a.at).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })}
                      </span>
                    </li>
                  )}
                </For>
              </ul>
            </Show>
          </div>
        </Popover>
      </Show>
    </div>
  );
}
