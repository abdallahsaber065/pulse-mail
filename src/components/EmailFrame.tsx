import { createMemo, onCleanup } from "solid-js";
import { emailSrcdoc, textSrcdoc } from "../lib/email-frame";
import { api } from "../lib/api";

interface Props {
  html: string | null;
  text: string;
  dark: boolean;
  allowRemoteImages: boolean;
}

/**
 * Sandboxed email body. `allow-same-origin` without `allow-scripts` lets us
 * size the frame and route link clicks to the system browser, while nothing
 * inside the email can execute.
 */
export default function EmailFrame(p: Props) {
  let frame!: HTMLIFrameElement;
  let observer: ResizeObserver | undefined;

  const srcdoc = createMemo(() => {
    const opts = { dark: p.dark, allowRemoteImages: p.allowRemoteImages };
    return p.html ? emailSrcdoc(p.html, opts) : textSrcdoc(p.text, opts);
  });

  const onLoad = () => {
    const doc = frame.contentDocument;
    if (!doc) return;
    const fit = () => {
      frame.style.height = `${Math.max(doc.documentElement.scrollHeight, doc.body?.scrollHeight ?? 0) + 8}px`;
    };
    fit();
    observer?.disconnect();
    observer = new ResizeObserver(fit);
    if (doc.body) observer.observe(doc.body);
    doc.addEventListener("click", (e) => {
      const a = (e.target as Element | null)?.closest?.("a[href]") as HTMLAnchorElement | null;
      if (!a) return;
      e.preventDefault();
      const href = a.getAttribute("href") ?? "";
      if (/^(https?:|mailto:)/i.test(href)) void api.openExternal(href).catch(() => {});
    });
  };

  onCleanup(() => observer?.disconnect());

  return (
    <iframe
      ref={frame}
      title="Email content"
      sandbox="allow-same-origin"
      referrerpolicy="no-referrer"
      class="selectable block w-full border-0"
      style={{ "min-height": "120px", "color-scheme": p.dark ? "dark" : "light" }}
      srcdoc={srcdoc()}
      onLoad={onLoad}
    />
  );
}
