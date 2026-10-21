// Anchored menu/popover surface: closes on outside click, Esc, or blur of the
// window. Render it inside a `relative` wrapper next to its trigger.
import { onCleanup, onMount, type JSX } from "solid-js";

export default function Popover(p: {
  onClose(): void;
  align?: "left" | "right";
  class?: string;
  label: string;
  children: JSX.Element;
}) {
  let el!: HTMLDivElement;
  onMount(() => {
    const onDown = (e: PointerEvent) => {
      // Clicks on the trigger (the wrapper) toggle it themselves.
      if (!el.parentElement?.contains(e.target as Node)) p.onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        p.onClose();
      }
    };
    document.addEventListener("pointerdown", onDown, true);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", p.onClose);
    onCleanup(() => {
      document.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("blur", p.onClose);
    });
  });
  return (
    <div
      ref={el}
      role="dialog"
      aria-label={p.label}
      class={`absolute top-full z-50 mt-2 origin-top rounded-2xl bg-surface-high text-text shadow-xl ring-1 ring-black/5 animate-pop-in ${
        p.align === "left" ? "left-0" : "right-0"
      } ${p.class ?? ""}`}
    >
      {p.children}
    </div>
  );
}
