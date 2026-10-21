// History download progress (H1 v2): phase, exact counts, smoothed rate and
// ETA, sizes, pause/resume, and Retry when it stopped after a failure.
import { Show } from "solid-js";
import Icon from "./Icon";
import { historyBar, historyLine, phaseLabel, sizeLine } from "../lib/accounts";
import { formatSize } from "../lib/format";
import type { History } from "../lib/types";

export default function HistoryProgress(p: { history: History; onPause(pause: boolean): void; onRetry(): void; compact?: boolean }) {
  const h = () => p.history;
  const bar = () => historyBar(h());
  return (
    <div class={`rounded-xl bg-surface-container ${p.compact ? "px-3 py-2" : "mb-3 px-3 py-2"}`}>
      <div class="mb-1 flex items-center gap-2 text-xs">
        <span class={`min-w-0 flex-1 truncate ${h().error ? "text-danger" : "text-text"}`}>
          {h().state === "done" ? "Download complete" : phaseLabel(h())}
        </span>
        <span class="shrink-0 tabular-nums text-muted">{historyLine(h())}</span>
        <Show when={h().state !== "done"}>
          <button
            class="grid size-6 shrink-0 place-items-center rounded-full text-muted hover:bg-hover"
            title={h().state === "paused" ? "Resume download" : "Pause download"}
            aria-label={h().state === "paused" ? "Resume download" : "Pause download"}
            onClick={() => p.onPause(h().state !== "paused")}
          >
            <Icon name={h().state === "paused" ? "play" : "pause"} size={16} />
          </button>
        </Show>
      </div>
      <Show when={h().state !== "done"}>
        <div class="h-1 overflow-hidden rounded-full bg-surface" role="progressbar" aria-label="Download progress">
          <Show when={bar() != null} fallback={<div class="progress-indeterminate h-full bg-primary" />}>
            <div
              class={`h-full rounded-full transition-[width] duration-300 ${h().state === "paused" || h().error ? "bg-outline" : "bg-primary"}`}
              style={{ width: `${Math.max(1, bar()! * 100)}%` }}
            />
          </Show>
        </div>
      </Show>
      <Show when={h().error}>
        <div class="mt-1.5 flex items-center gap-2 text-[11px] text-danger">
          <span class="min-w-0 flex-1">{h().error}</span>
          <button class="h-6 shrink-0 rounded-full px-2 font-medium text-primary hover:bg-primary/10" onClick={() => p.onRetry()}>
            Retry now
          </button>
        </div>
      </Show>
      <Show when={!h().error && sizeLine(h(), formatSize)}>
        <div class="mt-1 text-[11px] text-muted">{sizeLine(h(), formatSize)}</div>
      </Show>
    </div>
  );
}
