import { createSignal, createEffect, onCleanup, Show, Switch, Match } from "solid-js";
import Icon, { type IconName } from "./Icon";
import { formatSize } from "../lib/format";
import { api } from "../lib/api";
import { snack } from "./Snackbar";
import type { Attachment } from "../lib/types";

interface Props {
  messageId: string;
  attachment: Attachment;
  accountId: number | null;
  onClose(): void;
  onOpenApp(): void;
  onDownload(): void;
}

type PreviewKind = "image" | "pdf" | "text" | "audio" | "video" | "fallback";

function detectKind(mime: string, filename: string): PreviewKind {
  const m = mime.toLowerCase();
  const ext = filename.split(".").pop()?.toLowerCase() ?? "";

  if (m.startsWith("image/") || ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "avif"].includes(ext)) {
    return "image";
  }
  if (m === "application/pdf" || ext === "pdf") {
    return "pdf";
  }
  if (m.startsWith("audio/") || ["mp3", "wav", "ogg", "m4a", "flac", "aac"].includes(ext)) {
    return "audio";
  }
  if (m.startsWith("video/") || ["mp4", "webm", "ogg", "mov"].includes(ext)) {
    return "video";
  }
  if (
    m.startsWith("text/") ||
    m.includes("json") ||
    m.includes("csv") ||
    m.includes("xml") ||
    m.includes("javascript") ||
    m.includes("typescript") ||
    [
      "txt", "md", "markdown", "json", "csv", "tsv", "js", "jsx", "ts", "tsx",
      "py", "rs", "html", "htm", "css", "scss", "xml", "yml", "yaml", "toml",
      "ini", "conf", "sh", "bash", "bat", "ps1", "sql", "log", "diff", "patch",
      "env", "c", "cpp", "h", "hpp", "java", "go", "jsonc"
    ].includes(ext)
  ) {
    return "text";
  }
  return "fallback";
}

function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const len = bin.length;
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) {
    bytes[i] = bin.charCodeAt(i);
  }
  return bytes;
}

export default function AttachmentPreview(p: Props) {
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [objectUrl, setObjectUrl] = createSignal<string | null>(null);
  const [textContent, setTextContent] = createSignal<string | null>(null);
  const [wrapLines, setWrapLines] = createSignal(true);
  const [zoom, setZoom] = createSignal(1);

  const kind = () => detectKind(p.attachment.mime, p.attachment.filename);

  const fileIcon = (): IconName => {
    switch (kind()) {
      case "image":
        return "image";
      case "pdf":
        return "pdf";
      case "text":
        return "code";
      default:
        return "attachment";
    }
  };

  createEffect(() => {
    // Re-fetch when messageId or attachment changes
    const msgId = p.messageId;
    const part = p.attachment.part;
    const acct = p.accountId;

    setLoading(true);
    setError(null);
    setTextContent(null);
    setZoom(1);

    let active = true;

    api
      .getAttachmentData(msgId, part, acct)
      .then((data) => {
        if (!active) return;
        try {
          const bytes = b64ToBytes(data.dataBase64);
          const currentKind = detectKind(data.mime || p.attachment.mime, data.filename || p.attachment.filename);

          if (currentKind === "text") {
            const decoded = new TextDecoder("utf-8").decode(bytes);
            setTextContent(decoded);
          }

          const blob = new Blob([bytes as unknown as BlobPart], { type: data.mime || "application/octet-stream" });
          const url = URL.createObjectURL(blob);
          setObjectUrl((prev) => {
            if (prev) URL.revokeObjectURL(prev);
            return url;
          });
        } catch (err) {
          setError((err as Error).message || "Couldn't parse attachment data.");
        } finally {
          setLoading(false);
        }
      })
      .catch((err) => {
        if (!active) return;
        setError((err as Error).message || "Couldn't load attachment.");
        setLoading(false);
      });

    onCleanup(() => {
      active = false;
    });
  });

  onCleanup(() => {
    const url = objectUrl();
    if (url) URL.revokeObjectURL(url);
  });

  // Global escape key listener
  createEffect(() => {
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

  const copyText = async () => {
    const txt = textContent();
    if (!txt) return;
    try {
      await navigator.clipboard.writeText(txt);
      snack("Text copied to clipboard");
    } catch {
      snack("Couldn't copy text");
    }
  };

  return (
    <div
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-md p-4 animate-fade-in"
      onClick={p.onClose}
    >
      <div
        class="flex flex-col w-full max-w-5xl h-[85vh] max-h-[880px] rounded-2xl bg-surface border border-divider shadow-2xl overflow-hidden animate-pop-in"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header Bar */}
        <header class="flex h-14 shrink-0 items-center justify-between gap-3 border-b border-divider bg-surface-high/80 px-4">
          <div class="flex items-center gap-3 min-w-0">
            <span class="grid size-9 shrink-0 place-items-center rounded-xl bg-surface-dim border border-divider text-primary">
              <Icon name={fileIcon()} size={20} />
            </span>
            <div class="min-w-0">
              <div class="truncate text-sm font-semibold text-text" title={p.attachment.filename} dir="auto">
                {p.attachment.filename}
              </div>
              <div class="flex items-center gap-2 text-xs text-muted">
                <span>{formatSize(p.attachment.size)}</span>
                <span class="text-faint">•</span>
                <span class="truncate max-w-[160px] font-mono text-[11px] text-faint uppercase">
                  {p.attachment.filename.split(".").pop() || "FILE"}
                </span>
              </div>
            </div>
          </div>

          <div class="flex items-center gap-1.5">
            {/* Specific tool buttons */}
            <Show when={kind() === "text" && textContent() != null}>
              <button
                type="button"
                class={`flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-xs transition-colors ${
                  wrapLines() ? "bg-primary/10 text-primary font-medium" : "text-muted hover:bg-hover hover:text-text"
                }`}
                title="Toggle line wrap"
                onClick={() => setWrapLines((w) => !w)}
              >
                <Icon name="wrapText" size={16} />
                <span class="hidden sm:inline">Wrap</span>
              </button>
              <button
                type="button"
                class="flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-xs text-muted hover:bg-hover hover:text-text transition-colors"
                title="Copy all content"
                onClick={copyText}
              >
                <Icon name="copy" size={16} />
                <span class="hidden sm:inline">Copy</span>
              </button>
            </Show>

            <Show when={kind() === "image" && objectUrl() != null}>
              <div class="flex items-center gap-0.5 rounded-lg border border-divider p-0.5 bg-surface-dim">
                <button
                  type="button"
                  class="grid size-7 place-items-center rounded text-muted hover:bg-hover hover:text-text text-sm font-bold"
                  title="Zoom in"
                  onClick={() => setZoom((z) => Math.min(z + 0.25, 3))}
                >
                  +
                </button>
                <span class="px-1.5 text-[11px] text-faint font-mono">{Math.round(zoom() * 100)}%</span>
                <button
                  type="button"
                  class="grid size-7 place-items-center rounded text-muted hover:bg-hover hover:text-text text-sm font-bold"
                  title="Zoom out"
                  onClick={() => setZoom((z) => Math.max(z - 0.25, 0.5))}
                >
                  -
                </button>
                <button
                  type="button"
                  class="px-1.5 py-1 text-[11px] text-muted hover:bg-hover hover:text-text rounded transition-colors"
                  title="Reset zoom"
                  onClick={() => setZoom(1)}
                >
                  Reset
                </button>
              </div>
            </Show>

            <div class="mx-1 h-5 w-px bg-divider" />

            {/* Open with default app */}
            <button
              type="button"
              class="flex h-8 items-center gap-1.5 rounded-lg border border-divider px-2.5 text-xs text-text hover:bg-hover transition-colors"
              title="Open with system application"
              onClick={p.onOpenApp}
            >
              <Icon name="openExternal" size={16} class="text-muted" />
              <span class="hidden sm:inline">Open in App</span>
            </button>

            {/* Download */}
            <button
              type="button"
              class="flex h-8 items-center gap-1.5 rounded-lg border border-divider px-2.5 text-xs text-text hover:bg-hover transition-colors"
              title="Save to Downloads"
              onClick={p.onDownload}
            >
              <Icon name="download" size={16} class="text-muted" />
              <span class="hidden sm:inline">Download</span>
            </button>

            {/* Close */}
            <button
              type="button"
              class="grid size-8 place-items-center rounded-lg text-muted hover:bg-hover hover:text-text transition-colors ml-1"
              title="Close preview (Esc)"
              onClick={p.onClose}
            >
              <Icon name="close" size={18} />
            </button>
          </div>
        </header>

        {/* Content Body */}
        <div class="relative flex-1 min-h-0 bg-surface-dim/40 overflow-hidden flex flex-col items-center justify-center">
          <Show when={loading()}>
            <div class="flex flex-col items-center gap-3 p-8 text-center animate-pulse">
              <div class="size-10 rounded-full border-2 border-primary border-t-transparent animate-spin" />
              <div class="text-sm text-muted">Loading preview...</div>
            </div>
          </Show>

          <Show when={!loading() && error()}>
            <div class="flex flex-col items-center gap-4 max-w-md p-6 text-center">
              <span class="grid size-12 place-items-center rounded-full bg-danger/10 text-danger">
                <Icon name="error" size={28} />
              </span>
              <div>
                <div class="text-base font-semibold text-text">Couldn't preview this file</div>
                <div class="mt-1 text-xs text-muted">{error()}</div>
              </div>
              <div class="flex items-center gap-2">
                <button
                  type="button"
                  class="rounded-xl bg-primary px-4 py-2 text-xs font-semibold text-primary-fg shadow-sm hover:opacity-90 transition-opacity"
                  onClick={p.onOpenApp}
                >
                  Open in Default App
                </button>
                <button
                  type="button"
                  class="rounded-xl border border-divider px-4 py-2 text-xs font-semibold text-text hover:bg-hover transition-colors"
                  onClick={p.onDownload}
                >
                  Save to Downloads
                </button>
              </div>
            </div>
          </Show>

          <Show when={!loading() && !error() && objectUrl()}>
            <Switch>
              {/* Image Preview */}
              <Match when={kind() === "image"}>
                <div class="relative w-full h-full flex items-center justify-center overflow-auto p-4 select-none">
                  <img
                    src={objectUrl()!}
                    alt={p.attachment.filename}
                    class="max-h-full max-w-full object-contain rounded-lg shadow-sm transition-transform duration-150"
                    style={{ transform: `scale(${zoom()})` }}
                  />
                </div>
              </Match>

              {/* PDF Preview */}
              <Match when={kind() === "pdf"}>
                <iframe
                  src={`${objectUrl()!}#toolbar=1&navpanes=0`}
                  class="w-full h-full border-0 bg-white"
                  title={p.attachment.filename}
                />
              </Match>

              {/* Text / Code Preview */}
              <Match when={kind() === "text"}>
                <div class="w-full h-full overflow-auto bg-surface-high/30 p-4 font-mono text-xs text-text selection:bg-primary/20">
                  <pre
                    class={`tab-size-2 leading-relaxed ${
                      wrapLines() ? "whitespace-pre-wrap break-words" : "whitespace-pre"
                    }`}
                  >
                    <code>{textContent() || "(Empty file)"}</code>
                  </pre>
                </div>
              </Match>

              {/* Audio Preview */}
              <Match when={kind() === "audio"}>
                <div class="flex flex-col items-center gap-6 p-8 rounded-2xl bg-surface border border-divider shadow-md max-w-md w-full">
                  <span class="grid size-16 place-items-center rounded-2xl bg-primary/10 text-primary">
                    <Icon name="attachment" size={32} />
                  </span>
                  <div class="text-center">
                    <div class="font-semibold text-text truncate max-w-xs">{p.attachment.filename}</div>
                    <div class="text-xs text-faint mt-1">{formatSize(p.attachment.size)}</div>
                  </div>
                  <audio controls src={objectUrl()!} class="w-full" autoplay={false} />
                </div>
              </Match>

              {/* Video Preview */}
              <Match when={kind() === "video"}>
                <div class="w-full h-full flex items-center justify-center p-4">
                  <video
                    controls
                    src={objectUrl()!}
                    class="max-h-full max-w-full rounded-xl shadow-lg bg-black"
                    preload="metadata"
                  />
                </div>
              </Match>

              {/* Unsupported / Fallback Card */}
              <Match when={kind() === "fallback"}>
                <div class="flex flex-col items-center gap-5 p-8 rounded-2xl bg-surface border border-divider shadow-md max-w-md text-center">
                  <span class="grid size-16 place-items-center rounded-2xl bg-surface-dim border border-divider text-muted">
                    <Icon name="attachment" size={32} />
                  </span>
                  <div>
                    <div class="text-base font-semibold text-text truncate max-w-xs">{p.attachment.filename}</div>
                    <div class="text-xs text-muted mt-1">
                      {formatSize(p.attachment.size)} • {p.attachment.mime || "Binary file"}
                    </div>
                    <div class="text-xs text-faint mt-3">
                      In-app preview isn't available for this file type. You can open it in your system's default app or download it.
                    </div>
                  </div>
                  <div class="flex items-center gap-3 mt-2">
                    <button
                      type="button"
                      class="flex items-center gap-2 rounded-xl bg-primary px-4 py-2.5 text-xs font-semibold text-primary-fg shadow-sm hover:opacity-90 transition-opacity"
                      onClick={p.onOpenApp}
                    >
                      <Icon name="openExternal" size={16} />
                      <span>Open in Default App</span>
                    </button>
                    <button
                      type="button"
                      class="flex items-center gap-2 rounded-xl border border-divider px-4 py-2.5 text-xs font-semibold text-text hover:bg-hover transition-colors"
                      onClick={p.onDownload}
                    >
                      <Icon name="download" size={16} />
                      <span>Save to Downloads</span>
                    </button>
                  </div>
                </div>
              </Match>
            </Switch>
          </Show>
        </div>
      </div>
    </div>
  );
}
