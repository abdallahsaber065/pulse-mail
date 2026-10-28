import { createEffect, createSignal, on, onCleanup, onMount, Show } from "solid-js";
import { api } from "../lib/api";
import type { RenderedMessage } from "../lib/types";

interface Props {
  messageId: string;
  accountId: number | null;
  dark: boolean;
  showImages: boolean;
  /** Re-render when this changes (e.g. after allowlisting the sender). */
  version: number;
  onRendered(r: RenderedMessage): void;
  onLink(href: string, text: string): void;
}

/**
 * Sandboxed email body.
 *
 * The document comes from Rust already sanitized (ammonia), with a strict CSP
 * and no remote resources. The iframe is `sandbox="allow-scripts"` WITHOUT
 * `allow-same-origin`: it runs in an opaque origin, so nothing in it can
 * reach the app, its storage or the IPC bridge. The only script the CSP
 * allows is our own (hash-pinned); it posts the content height and link
 * clicks here, and folds quoted text.
 */
export default function EmailFrame(p: Props) {
  let frame!: HTMLIFrameElement;
  const [doc, setDoc] = createSignal<string | null>(null);
  const [height, setHeight] = createSignal(120);
  const [error, setError] = createSignal<string | null>(null);

  let seq = 0;
  createEffect(
    on(
      () => [p.messageId, p.dark, p.showImages, p.version] as const,
      ([id, dark, images], prev) => {
        const n = ++seq;
        setError(null);
        // A reused row (Index re-purposes it when the thread changes) must
        // never show the previous message's body while this one renders.
        if (prev && prev[0] !== id) {
          setDoc(null);
          setHeight(120);
        }
        api.renderMessage(id, dark, images, p.accountId).then(
          (r) => {
            if (n !== seq) return;
            setDoc(r.srcdoc);
            p.onRendered(r);
          },
          (e: Error) => n === seq && setError(e.message),
        );
      },
    ),
  );

  onMount(() => {
    const onMessage = (e: MessageEvent) => {
      if (e.source !== frame?.contentWindow) return;
      const d = e.data as { pm?: number; t?: string; v?: unknown; href?: unknown; text?: unknown } | null;
      if (!d || d.pm !== 1) return;
      if (d.t === "h" && typeof d.v === "number" && Number.isFinite(d.v)) {
        if (document.body.classList.contains("win-hidden")) return;
        const next = Math.min(Math.max(Math.ceil(d.v), 40), 16_000);
        setHeight((prev) => {
          // Break runaway feedback loops: ignore tiny fluctuations <= 4px
          if (Math.abs(next - prev) <= 4) return prev;
          return next;
        });
      } else if (d.t === "link" && typeof d.href === "string") {
        p.onLink(d.href.slice(0, 4096), typeof d.text === "string" ? d.text.slice(0, 300) : "");
      }
    };
    window.addEventListener("message", onMessage);
    onCleanup(() => window.removeEventListener("message", onMessage));
  });

  return (
    <Show when={!error()} fallback={<p class="text-sm text-danger">{error()}</p>}>
      <Show when={doc()} fallback={<div class="skeleton h-24 rounded-xl" aria-label="Rendering message" />}>
        <iframe
          ref={frame}
          title="Email content"
          sandbox="allow-scripts"
          referrerpolicy="no-referrer"
          class={`email-frame block w-full border-0 bg-transparent ${p.dark ? "rounded-xl" : ""}`}
          style={{ height: `${height()}px` }}
          srcdoc={doc()!}
        />
      </Show>
    </Show>
  );
}
