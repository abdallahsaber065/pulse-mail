// Material 3 snackbar queue: one at a time, optional action (e.g. Undo),
// slide-in/out, pauses while hovered.
import { createEffect, createSignal, onCleanup, Show } from "solid-js";
import Icon from "./Icon";

export interface SnackOptions {
  action?: { label: string; run(): void };
  /** ms; default 4000, 6000 with an action. */
  duration?: number;
}

interface Snack extends SnackOptions {
  id: number;
  text: string;
}

const [queue, setQueue] = createSignal<Snack[]>([]);
const [leaving, setLeaving] = createSignal(false);
let nextId = 1;

export function snack(text: string, opts: SnackOptions = {}) {
  const item: Snack = { id: nextId++, text, ...opts };
  // A newer message with the same text replaces the old one.
  setQueue((q) => [...q.filter((s) => s.text !== text || s === q[0]), item]);
  if (queue()[0] !== item && queue()[0]?.text === text) dismiss();
}

function dismiss() {
  if (leaving() || queue().length === 0) return;
  setLeaving(true);
  window.setTimeout(() => {
    setQueue((q) => q.slice(1));
    setLeaving(false);
  }, 160);
}

export function SnackbarHost() {
  let timer: number | undefined;
  let shownId = 0;
  let paused = false;
  const head = () => queue()[0];
  createEffect(() => {
    const s = head();
    if (s && s.id !== shownId) {
      shownId = s.id;
      paused = false;
      arm(s);
    }
  });
  const arm = (s: Snack) => {
    clearTimeout(timer);
    // Another message waiting: shorten this one.
    const base = s.duration ?? (s.action ? 6000 : 4000);
    timer = window.setTimeout(() => !paused && dismiss(), queue().length > 1 ? Math.min(base, 2000) : base);
  };
  onCleanup(() => clearTimeout(timer));

  return (
    <div class="pointer-events-none fixed bottom-6 left-6 z-[70]" aria-live="polite">
      <Show when={head()} keyed>
        {(s) => (
          <div
            role="status"
            class={`pointer-events-auto flex min-h-12 min-w-[288px] max-w-[560px] items-center gap-2 rounded-md bg-inverse-surface py-1 pl-4 pr-2 text-sm text-inverse-on-surface shadow-lg ${
              leaving() ? "animate-snack-out" : "animate-snack-in"
            }`}
            onMouseEnter={() => {
              paused = true;
              clearTimeout(timer);
            }}
            onMouseLeave={() => {
              paused = false;
              arm(s);
            }}
          >
            <span class="flex-1 py-2">{s.text}</span>
            <Show when={s.action}>
              <button
                class="h-9 rounded-full px-3 font-medium text-inverse-primary hover:bg-white/10"
                onClick={() => {
                  s.action!.run();
                  dismiss();
                }}
              >
                {s.action!.label}
              </button>
            </Show>
            <button class="grid size-9 place-items-center rounded-full hover:bg-white/10" title="Dismiss" onClick={dismiss}>
              <Icon name="close" size={18} />
            </button>
          </div>
        )}
      </Show>
    </div>
  );
}
