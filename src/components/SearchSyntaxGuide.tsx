import { createSignal, For } from "solid-js";
import Icon from "./Icon";
import { SYNTAX_GUIDE, type SyntaxDoc } from "../lib/search-syntax";

interface Props {
  onSelect(token: string): void;
  onClose(): void;
}

const CATEGORIES: Array<SyntaxDoc["category"]> = [
  "Filters",
  "People & Subject",
  "Location",
  "Attachments",
  "Date & Size",
];

export default function SearchSyntaxGuide(p: Props) {
  const [activeCategory, setActiveCategory] = createSignal<SyntaxDoc["category"]>("Filters");

  const filtered = () => SYNTAX_GUIDE.filter((s) => s.category === activeCategory());

  return (
    <div
      class="absolute left-0 right-0 top-full mt-2 z-50 max-h-[440px] w-full overflow-hidden rounded-2xl border border-divider bg-surface-high p-4 shadow-2xl animate-fade-in"
      role="dialog"
      aria-label="Search syntax guide"
      onPointerDown={(e) => e.stopPropagation()}
    >
      <div class="mb-3 flex items-center justify-between border-b border-divider pb-2.5">
        <div class="flex items-center gap-2 text-text font-medium text-sm">
          <Icon name="info" size={18} class="text-primary" />
          <span>Supported Search Operators</span>
        </div>
        <button
          class="grid size-7 place-items-center rounded-full text-muted hover:bg-hover hover:text-text"
          onClick={p.onClose}
          title="Close guide"
          aria-label="Close"
        >
          <Icon name="close" size={16} />
        </button>
      </div>

      {/* Categories Navigation Pills */}
      <div class="mb-3 flex flex-wrap gap-1.5">
        <For each={CATEGORIES}>
          {(cat) => (
            <button
              class={`rounded-full px-3 py-1 text-xs font-medium transition-colors ${
                activeCategory() === cat
                  ? "bg-primary text-on-primary"
                  : "bg-surface-container text-muted hover:bg-hover hover:text-text"
              }`}
              onClick={() => setActiveCategory(cat)}
            >
              {cat}
            </button>
          )}
        </For>
      </div>

      {/* Syntax List for Active Category */}
      <div class="max-h-[280px] overflow-y-auto space-y-1.5 pr-1">
        <For each={filtered()}>
          {(doc) => (
            <div
              class="group flex cursor-pointer items-center justify-between rounded-xl p-2 text-left hover:bg-hover transition-colors"
              onClick={() => {
                p.onSelect(doc.example);
                p.onClose();
              }}
              title={`Click to insert: ${doc.example}`}
            >
              <div class="min-w-0 flex-1">
                <div class="flex items-baseline gap-2">
                  <span class="font-mono text-xs font-bold text-primary">{doc.syntax}</span>
                  <span class="text-xs text-text truncate">{doc.description}</span>
                </div>
                <div class="text-[11px] text-faint font-mono">Example: {doc.example}</div>
              </div>
              <span class="opacity-0 group-hover:opacity-100 text-xs font-medium text-primary px-2 py-0.5 rounded bg-primary/10 transition-opacity">
                Insert
              </span>
            </div>
          )}
        </For>
      </div>

      <div class="mt-3 border-t border-divider pt-2 flex items-center justify-between text-[11px] text-faint">
        <span>Click any operator to insert it into your search box.</span>
        <span>Press <kbd class="px-1 py-0.5 rounded bg-surface-container font-mono">Esc</kbd> to close</span>
      </div>
    </div>
  );
}
