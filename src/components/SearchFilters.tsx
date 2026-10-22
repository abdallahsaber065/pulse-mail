import { For } from "solid-js";
import { hasSearchFilter, toggleSearchFilter, type SearchFilter } from "../lib/search";

export default function SearchFilters(p: { query: string; disabled: boolean; onQuery(q: string): void }) {
  const filters: [SearchFilter, string][] = [["is:unread", "Unread"], ["is:starred", "Starred"], ["has:attachment", "Has attachment"]];
  return (
    <div class="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-divider px-4 py-2" role="group" aria-label="Search all mail">
      <span class="mr-1 text-xs text-muted">Search all mail</span>
      <For each={filters}>{([filter, label]) => (
        <button
          class={`min-h-8 rounded-full border px-3 text-xs transition-colors disabled:opacity-50 ${hasSearchFilter(p.query, filter) ? "border-primary bg-secondary-container text-on-secondary-container" : "border-outline text-muted hover:bg-hover"}`}
          aria-pressed={hasSearchFilter(p.query, filter)}
          disabled={p.disabled}
          onClick={() => p.onQuery(toggleSearchFilter(p.query, filter))}
        >{label}</button>
      )}</For>
    </div>
  );
}
