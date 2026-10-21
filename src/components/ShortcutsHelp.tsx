import { For, onCleanup, onMount } from "solid-js";
import Icon from "./Icon";
import { SHORTCUTS } from "../lib/shortcuts";

export default function ShortcutsHelp(p: { onClose(): void }) {
  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" || e.key === "?") {
        e.preventDefault();
        e.stopPropagation();
        p.onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    onCleanup(() => window.removeEventListener("keydown", onKey, true));
  });
  return (
    <div class="fixed inset-0 z-50 grid place-items-center bg-black/40 animate-fade-in" onClick={p.onClose} role="dialog" aria-modal="true" aria-label="Keyboard shortcuts">
      <div class="w-[440px] rounded-[28px] bg-surface-high p-6 shadow-2xl animate-dialog-in" onClick={(e) => e.stopPropagation()}>
        <div class="mb-4 flex items-center">
          <h2 class="flex-1 font-display text-xl text-text">Keyboard shortcuts</h2>
          <button class="grid size-9 place-items-center rounded-full text-muted hover:bg-hover" onClick={p.onClose} title="Close">
            <Icon name="close" />
          </button>
        </div>
        <dl class="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-2 text-sm">
          <For each={SHORTCUTS}>
            {([keys, what]) => (
              <>
                <dt>
                  <kbd class="rounded-md border border-outline bg-surface-container px-1.5 py-0.5 font-mono text-xs">{keys}</kbd>
                </dt>
                <dd class="text-muted">{what}</dd>
              </>
            )}
          </For>
        </dl>
      </div>
    </div>
  );
}
