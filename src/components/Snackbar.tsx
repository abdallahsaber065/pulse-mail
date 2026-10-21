import { Show } from "solid-js";

export default function Snackbar(p: { text: string | null }) {
  return (
    <Show when={p.text}>
      <div class="fixed bottom-6 left-6 z-50 rounded-lg bg-[#303030] px-4 py-3 text-sm text-white shadow-lg" role="status">
        {p.text}
      </div>
    </Show>
  );
}
