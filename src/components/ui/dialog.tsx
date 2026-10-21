// Material 3 basic dialog with scrim + enter/exit animation, driven by a tiny
// promise API so callers read like `if (await confirmDialog({...})) ...`.
// Replaces window.confirm/alert/prompt everywhere.
import { createSignal, For, onCleanup, onMount, Show, type JSX } from "solid-js";
import Icon, { type IconName } from "../Icon";

interface DialogButton {
  label: string;
  value: string;
  kind?: "text" | "filled" | "danger";
}

interface DialogRequest {
  title: string;
  body?: JSX.Element | string;
  icon?: IconName;
  buttons: DialogButton[];
  /** Returned on Esc / scrim click. */
  dismissValue: string;
  resolve(value: string): void;
}

const [current, setCurrent] = createSignal<DialogRequest | null>(null);
const [closing, setClosing] = createSignal(false);

export function showDialog(opts: Omit<DialogRequest, "resolve">): Promise<string> {
  // A new dialog replaces any open one (which resolves as dismissed).
  current()?.resolve(current()!.dismissValue);
  setClosing(false);
  return new Promise((resolve) => setCurrent({ ...opts, resolve }));
}

export async function confirmDialog(opts: {
  title: string;
  body?: JSX.Element | string;
  icon?: IconName;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
}): Promise<boolean> {
  const v = await showDialog({
    title: opts.title,
    body: opts.body,
    icon: opts.icon,
    dismissValue: "cancel",
    buttons: [
      { label: opts.cancelLabel ?? "Cancel", value: "cancel", kind: "text" },
      { label: opts.confirmLabel ?? "OK", value: "ok", kind: opts.danger ? "danger" : "filled" },
    ],
  });
  return v === "ok";
}

function close(value: string) {
  const req = current();
  if (!req || closing()) return;
  setClosing(true);
  window.setTimeout(() => {
    if (current() === req) {
      setCurrent(null);
      setClosing(false);
    }
    req.resolve(value);
  }, 140);
}

export function DialogHost() {
  return (
    <Show when={current()} keyed>
      {(req) => <DialogView req={req} />}
    </Show>
  );
}

function DialogView(p: { req: DialogRequest }) {
  let panel!: HTMLDivElement;
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      close(p.req.dismissValue);
    } else if (e.key === "Tab") {
      // Keep focus inside the dialog.
      const items = [...panel.querySelectorAll<HTMLElement>("button")];
      const i = items.indexOf(document.activeElement as HTMLElement);
      if (e.shiftKey && i <= 0) {
        e.preventDefault();
        items[items.length - 1]?.focus();
      } else if (!e.shiftKey && i === items.length - 1) {
        e.preventDefault();
        items[0]?.focus();
      }
    }
  };
  onMount(() => {
    const prev = document.activeElement as HTMLElement | null;
    window.addEventListener("keydown", onKey, true);
    // Focus the safe choice (first button) so Enter never confirms a destructive action by accident.
    panel.querySelector<HTMLElement>("button")?.focus();
    onCleanup(() => {
      window.removeEventListener("keydown", onKey, true);
      prev?.focus?.();
    });
  });

  const btnClass = (k: DialogButton["kind"]) =>
    k === "danger"
      ? "bg-danger text-on-danger hover:shadow-md"
      : k === "filled"
        ? "bg-primary text-on-primary hover:shadow-md"
        : "text-primary hover:bg-primary/10";

  return (
    <div class="fixed inset-0 z-[60] grid place-items-center p-6" role="presentation">
      <div
        class={`absolute inset-0 bg-black/40 ${closing() ? "animate-fade-out" : "animate-fade-in"}`}
        onClick={() => close(p.req.dismissValue)}
      />
      <div
        ref={panel}
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="dlg-title"
        class={`relative w-full max-w-[400px] rounded-[28px] bg-surface-high p-6 text-text shadow-2xl ${
          closing() ? "animate-dialog-out" : "animate-dialog-in"
        }`}
      >
        <Show when={p.req.icon}>
          <div class="mb-4 flex justify-center text-secondary">
            <Icon name={p.req.icon!} size={24} />
          </div>
        </Show>
        <h2 id="dlg-title" class={`font-display text-2xl leading-8 ${p.req.icon ? "text-center" : ""}`}>
          {p.req.title}
        </h2>
        <Show when={p.req.body}>
          <div class="mt-4 text-sm leading-5 text-muted">{p.req.body}</div>
        </Show>
        <div class="mt-6 flex justify-end gap-2">
          <For each={p.req.buttons}>
            {(b) => (
              <button
                class={`h-10 rounded-full px-6 text-sm font-medium transition-shadow ${btnClass(b.kind)}`}
                onClick={() => close(b.value)}
              >
                {b.label}
              </button>
            )}
          </For>
        </div>
      </div>
    </div>
  );
}
