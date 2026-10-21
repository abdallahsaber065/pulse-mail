import { createMemo, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import Icon, { type IconName } from "./Icon";
import { rank, type Command } from "../lib/palette";

/** Ctrl+K: search every command, label and account; Enter runs, Esc closes. */
export default function CommandPalette(p: { commands: Command[]; onSearch(q: string): void; onClose(): void }) {
  const [q, setQ] = createSignal("");
  const [active, setActive] = createSignal(0);
  let input: HTMLInputElement | undefined;
  let list: HTMLDivElement | undefined;

  const results = createMemo(() => {
    const r = rank(q(), p.commands);
    const text = q().trim();
    if (!text) return r;
    const search: Command = {
      id: "search",
      group: "Search",
      title: `Search mail for "${text}"`,
      icon: "search",
      run: () => p.onSearch(text),
    };
    return r.length ? [...r, search] : [search];
  });

  const run = (c: Command | undefined) => {
    if (!c) return;
    p.onClose();
    // Let the palette unmount (and release focus) before the command runs.
    queueMicrotask(() => c.run());
  };

  const move = (d: number) => {
    const n = results().length;
    if (!n) return;
    setActive((i) => (i + d + n) % n);
    list?.querySelector(`[data-i="${active()}"]`)?.scrollIntoView({ block: "nearest" });
  };

  onMount(() => {
    input?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        p.onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    onCleanup(() => window.removeEventListener("keydown", onKey, true));
  });

  return (
    <div class="fixed inset-0 z-50 flex justify-center bg-black/40 pt-[12vh] animate-fade-in" onClick={p.onClose} role="dialog" aria-modal="true" aria-label="Command palette">
      <div class="flex h-fit max-h-[70vh] w-[560px] max-w-[92vw] flex-col overflow-hidden rounded-[28px] bg-surface-high shadow-2xl animate-dialog-in" onClick={(e) => e.stopPropagation()}>
        <div class="flex h-14 shrink-0 items-center gap-3 border-b border-divider px-5">
          <Icon name="search" class="text-muted" />
          <input
            ref={input}
            class="no-ring h-full flex-1 bg-transparent text-base text-text outline-none placeholder:text-faint"
            placeholder="Type a command, label or account…"
            value={q()}
            onInput={(e) => {
              setQ(e.currentTarget.value);
              setActive(0);
            }}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                move(1);
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                move(-1);
              } else if (e.key === "Enter") {
                e.preventDefault();
                run(results()[active()]);
              }
            }}
            aria-controls="palette-list"
            aria-activedescendant={`palette-${active()}`}
            role="combobox"
            aria-expanded="true"
          />
          <kbd class="rounded-md border border-outline px-1.5 py-0.5 font-mono text-[11px] text-faint">Esc</kbd>
        </div>
        <div ref={list} id="palette-list" class="min-h-0 overflow-y-auto py-2" role="listbox">
          <For each={results()}>
            {(c, i) => (
              <>
                <Show when={i() === 0 || results()[i() - 1].group !== c.group}>
                  <div class="px-5 pb-1 pt-2 text-[11px] font-medium uppercase tracking-wide text-faint">{c.group}</div>
                </Show>
                <div
                  id={`palette-${i()}`}
                  data-i={i()}
                  role="option"
                  aria-selected={i() === active()}
                  class={`mx-2 flex h-10 cursor-pointer items-center gap-3 rounded-xl px-3 text-sm ${i() === active() ? "bg-secondary-container text-text" : "text-text hover:bg-hover"}`}
                  onMouseMove={() => setActive(i())}
                  onClick={() => run(c)}
                >
                  <Icon name={(c.icon ?? "arrowForward") as IconName} size={18} class="text-muted" />
                  <span class="min-w-0 flex-1 truncate">{c.title}</span>
                  <Show when={c.hint}>
                    <span class="truncate text-xs text-faint">{c.hint}</span>
                  </Show>
                  <Show when={c.keys}>
                    <kbd class="rounded-md border border-outline px-1.5 py-0.5 font-mono text-[11px] text-faint">{c.keys}</kbd>
                  </Show>
                </div>
              </>
            )}
          </For>
          <Show when={results().length === 0}>
            <div class="px-5 py-6 text-center text-sm text-muted">No matching commands</div>
          </Show>
        </div>
      </div>
    </div>
  );
}
