import { For } from "solid-js";
import Icon from "./Icon";
import { SHORTCUTS } from "../lib/shortcuts";

export default function ShortcutsHelp(p: { onClose(): void }) {
  return (
    <div class="fixed inset-0 z-50 grid place-items-center bg-black/40" onClick={p.onClose} role="dialog" aria-label="Keyboard shortcuts">
      <div class="w-[440px] rounded-3xl bg-surface p-6 shadow-xl" onClick={(e) => e.stopPropagation()}>
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
