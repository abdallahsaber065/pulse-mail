import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import Icon from "./Icon";
import { api } from "../lib/api";
import { attachTotal, hasContent, MAX_ATTACH_TOTAL, sameContent, textToHtml } from "../lib/compose";
import { formatSize } from "../lib/format";
import type { Account, Draft, DraftAttachment } from "../lib/types";
import RichEditor, { type EditorApi } from "./RichEditor";

const AUTOSAVE_MS = 2000;
/** Drafts with big files re-upload on every save: save less often. */
const AUTOSAVE_BIG_MS = 10_000;

interface Props {
  title: string;
  initial: Draft;
  /** Account the message is sent from. */
  accountId: number | null;
  accounts: Account[];
  /** New messages may pick the account; replies stay on the thread's. */
  canPickAccount: boolean;
  onSend(d: Draft, accountId: number | null): void;
  onClose(d: Draft, accountId: number | null, saved: boolean): void;
  onDiscard(d: Draft, accountId: number | null): void;
}

/** Docked compose sheet (bottom right): autosaves to Gmail Drafts. */
export default function Compose(p: Props) {
  const [d, setD] = createSignal<Draft>({ ...p.initial });
  const [account, setAccount] = createSignal<number | null>(p.accountId);
  const [showCc, setShowCc] = createSignal(!!(p.initial.cc || p.initial.bcc));
  const [showQuote, setShowQuote] = createSignal(false);
  const [status, setStatus] = createSignal<"" | "saving" | "saved" | "error">(p.initial.draftId ? "saved" : "");
  const [error, setError] = createSignal<string | null>(null);
  let saved: Draft | null = p.initial.draftId ? { ...p.initial } : null;
  let pending: Promise<void> | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let body: EditorApi | undefined;
  let to: HTMLInputElement | undefined;
  let picker: HTMLInputElement | undefined;
  const [uploading, setUploading] = createSignal(0);
  const [dragging, setDragging] = createSignal(false);

  const schedule = () => {
    setError(null);
    clearTimeout(timer);
    timer = setTimeout(() => void save(), attachTotal(d().attachments) > 5 * 1024 * 1024 ? AUTOSAVE_BIG_MS : AUTOSAVE_MS);
  };
  const set = (k: "to" | "cc" | "bcc" | "subject" | "body", v: string) => {
    setD((x) => ({ ...x, [k]: v }));
    schedule();
  };
  const setBody = (html: string, text: string) => {
    setD((x) => ({ ...x, html: html || null, body: text }));
    schedule();
  };

  /** Upload picked or dropped files (Gmail: 25 MB in total). */
  const addFiles = async (files: FileList | File[]) => {
    for (const f of Array.from(files)) {
      if (attachTotal(d().attachments) + f.size > MAX_ATTACH_TOTAL) {
        setError(`"${f.name}" doesn't fit: Gmail allows 25 MB of attachments per message.`);
        continue;
      }
      setUploading((n) => n + 1);
      try {
        const r = await api.stageAttachment(f);
        const a: DraftAttachment = { name: r.name, mime: r.mime, size: r.size, staged: r.id };
        setD((x) => ({ ...x, attachments: [...x.attachments, a] }));
        schedule();
      } catch (e) {
        setError((e as Error).message);
      } finally {
        setUploading((n) => n - 1);
      }
    }
  };
  const removeFile = (i: number) => {
    const a = d().attachments[i];
    if (a?.staged) void api.unstageAttachment(a.staged).catch(() => {});
    setD((x) => ({ ...x, attachments: x.attachments.filter((_, j) => j !== i) }));
    schedule();
  };

  /** Save to Gmail Drafts if anything changed since the last save. */
  const save = async (): Promise<boolean> => {
    if (pending) await pending;
    const cur = d();
    if (!hasContent(cur)) return false;
    if (saved && sameContent(saved, cur) && cur.draftId) return true;
    setStatus("saving");
    let ok = false;
    pending = api
      .saveDraft(cur, account())
      .then((r) => {
        setD((x) => ({ ...x, draftId: r.draftId, threadId: x.threadId ?? r.threadId }));
        saved = { ...cur, draftId: r.draftId };
        setStatus("saved");
        ok = true;
      })
      .catch((e) => {
        setStatus("error");
        setError((e as Error).message);
      })
      .finally(() => (pending = null));
    await pending;
    return ok;
  };

  const send = async () => {
    clearTimeout(timer);
    if (pending) await pending;
    if (uploading() > 0) {
      setError("Wait for the attachments to finish uploading.");
      return;
    }
    if (!d().to.trim() && !d().cc.trim() && !d().bcc.trim()) {
      setError("Add at least one recipient.");
      to?.focus();
      return;
    }
    p.onSend(d(), account());
  };

  const close = async () => {
    clearTimeout(timer);
    const ok = await save();
    p.onClose(d(), account(), ok);
  };

  const discard = async () => {
    clearTimeout(timer);
    if (pending) await pending;
    p.onDiscard(d(), account());
  };

  onMount(() => {
    if (p.initial.to) queueMicrotask(() => body?.focus(true));
    else to?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (!(e.target instanceof Node) || !root?.contains(e.target)) return;
      if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
        e.preventDefault();
        void send();
      } else if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        void close();
      }
    };
    window.addEventListener("keydown", onKey, true);
    onCleanup(() => {
      window.removeEventListener("keydown", onKey, true);
      clearTimeout(timer);
    });
  });
  let root: HTMLDivElement | undefined;

  const field = "no-ring h-10 w-full min-w-0 bg-transparent text-sm text-text outline-none placeholder:text-faint";
  const email = () => p.accounts.find((a) => a.id === account())?.email ?? p.accounts[0]?.email ?? "";

  return (
    <div
      ref={root}
      data-dropzone
      onDragOver={(e) => {
        if (e.dataTransfer?.types.includes("Files")) {
          e.preventDefault();
          setDragging(true);
        }
      }}
      onDragLeave={(e) => {
        if (!root?.contains(e.relatedTarget as Node | null)) setDragging(false);
      }}
      onDrop={(e) => {
        e.preventDefault();
        setDragging(false);
        if (e.dataTransfer?.files.length) void addFiles(e.dataTransfer.files);
      }}
      class="fixed bottom-0 right-6 z-40 flex h-[min(600px,calc(100vh-88px))] w-[560px] max-w-[calc(100vw-32px)] flex-col overflow-hidden rounded-t-2xl bg-surface shadow-2xl ring-1 ring-black/10 animate-dialog-in dark:ring-white/10"
      role="dialog"
      aria-label={p.title}
    >
      <div class="flex h-11 shrink-0 items-center gap-2 bg-surface-container pl-4 pr-1.5">
        <span class="flex-1 truncate text-sm font-medium text-text" dir="auto">{d().subject || p.title}</span>
        <span class="text-xs text-faint" aria-live="polite">
          {status() === "saving" ? "Saving…" : status() === "saved" ? "Draft saved" : status() === "error" ? "Not saved" : ""}
        </span>
        <button class="grid size-8 place-items-center rounded-full text-muted hover:bg-hover" title="Save & close (Esc)" onClick={() => void close()}>
          <Icon name="close" size={18} />
        </button>
      </div>

      <div class="flex shrink-0 items-center gap-2 border-b border-divider px-4">
        <span class="w-10 text-sm text-faint">From</span>
        <Show when={p.canPickAccount && p.accounts.length > 1} fallback={<span class="h-10 flex-1 truncate text-sm leading-10 text-muted">{email()}</span>}>
          <select
            class="no-ring h-10 flex-1 bg-transparent text-sm text-text outline-none"
            value={String(account() ?? p.accounts[0]?.id)}
            onChange={(e) => {
              setAccount(Number(e.currentTarget.value));
              // A draft belongs to one account; start a fresh one on the new account.
              setD((x) => ({ ...x, draftId: null }));
              saved = null;
            }}
          >
            <For each={p.accounts}>{(a) => <option value={a.id}>{a.email}</option>}</For>
          </select>
        </Show>
      </div>
      <div class="flex shrink-0 items-center gap-2 border-b border-divider px-4">
        <span class="w-10 text-sm text-faint">To</span>
        <input ref={to} class={field} value={d().to} onInput={(e) => set("to", e.currentTarget.value)} aria-label="To" spellcheck={false} />
        <Show when={!showCc()}>
          <button class="whitespace-nowrap text-sm text-muted hover:text-text" onClick={() => setShowCc(true)}>
            Cc Bcc
          </button>
        </Show>
      </div>
      <Show when={showCc()}>
        <div class="flex shrink-0 items-center gap-2 border-b border-divider px-4">
          <span class="w-10 text-sm text-faint">Cc</span>
          <input class={field} value={d().cc} onInput={(e) => set("cc", e.currentTarget.value)} aria-label="Cc" spellcheck={false} />
        </div>
        <div class="flex shrink-0 items-center gap-2 border-b border-divider px-4">
          <span class="w-10 text-sm text-faint">Bcc</span>
          <input class={field} value={d().bcc} onInput={(e) => set("bcc", e.currentTarget.value)} aria-label="Bcc" spellcheck={false} />
        </div>
      </Show>
      <div class="flex shrink-0 items-center border-b border-divider px-4">
        <input class={field} placeholder="Subject" value={d().subject} onInput={(e) => set("subject", e.currentTarget.value)} aria-label="Subject" dir="auto" />
      </div>

      <div class="flex min-h-0 flex-1 flex-col overflow-y-auto px-4 py-3">
        <RichEditor html={p.initial.html ?? textToHtml(p.initial.body)} onChange={setBody} ref={(api) => (body = api)} />
        <Show when={d().attachments.length || uploading()}>
          <div class="mt-2 flex flex-wrap gap-2">
            <For each={d().attachments}>
              {(a, i) => (
                <div class="flex h-8 max-w-[240px] items-center gap-1.5 rounded-lg bg-surface-container pl-2 pr-1 text-xs text-text">
                  <Icon name="attachment" size={16} class="shrink-0 text-muted" />
                  <span class="min-w-0 truncate" dir="auto">{a.name}</span>
                  <span class="shrink-0 text-faint">{formatSize(a.size)}</span>
                  <button class="grid size-6 shrink-0 place-items-center rounded-full text-muted hover:bg-hover" title={`Remove ${a.name}`} aria-label={`Remove ${a.name}`} onClick={() => removeFile(i())}>
                    <Icon name="close" size={14} />
                  </button>
                </div>
              )}
            </For>
            <Show when={uploading()}>
              <div class="flex h-8 items-center rounded-lg bg-surface-container px-3 text-xs text-muted">Attaching…</div>
            </Show>
          </div>
        </Show>
        <Show when={d().quote}>
          <div class="mt-2">
            <button
              class="rounded-md bg-surface-container px-2 py-0.5 text-xs text-muted hover:text-text"
              title={showQuote() ? "Hide quoted text" : "Show quoted text"}
              aria-expanded={showQuote()}
              onClick={() => setShowQuote((v) => !v)}
            >
              •••
            </button>
            <Show when={showQuote()}>
              <div class="selectable mt-2 whitespace-pre-wrap border-l-2 border-divider pl-3 text-xs leading-5 text-muted" dir="auto">
                {d().quote!.header}
                {"\n"}
                {d().quote!.text}
              </div>
              <button
                class="mt-1 text-xs text-primary hover:underline"
                onClick={() => {
                  setD((x) => ({ ...x, quote: null }));
                  schedule();
                }}
              >
                Remove quoted text
              </button>
            </Show>
          </div>
        </Show>
      </div>

      <Show when={error()}>
        <div class="mx-4 mb-2 rounded-lg bg-danger-container px-3 py-2 text-xs text-on-danger-container" role="alert">
          {error()}
        </div>
      </Show>
      <div class="flex h-14 shrink-0 items-center gap-2 border-t border-divider px-4">
        <button
          class="flex h-10 items-center gap-2 rounded-full bg-primary px-6 text-sm font-medium text-on-primary transition-shadow hover:shadow"
          onClick={() => void send()}
          title="Send (Ctrl+Enter)"
        >
          Send <Icon name="send" size={16} />
        </button>
        <span class="text-xs text-faint">Ctrl+Enter</span>
        <input
          ref={picker}
          type="file"
          multiple
          class="hidden"
          onChange={(e) => {
            if (e.currentTarget.files?.length) void addFiles(e.currentTarget.files);
            e.currentTarget.value = "";
          }}
        />
        <button class="ml-2 grid size-9 place-items-center rounded-full text-muted hover:bg-hover" title="Attach files (or drop them here)" aria-label="Attach files" onClick={() => picker?.click()}>
          <Icon name="attach" size={20} />
        </button>
        <Show when={d().attachments.length}>
          <span class="text-xs text-faint">{formatSize(attachTotal(d().attachments))} of 25 MB</span>
        </Show>
        <button class="ml-auto grid size-9 place-items-center rounded-full text-muted hover:bg-hover" title="Discard draft" onClick={() => void discard()}>
          <Icon name="trash" size={18} />
        </button>
      </div>
      <Show when={dragging()}>
        <div class="pointer-events-none absolute inset-0 grid place-items-center rounded-t-2xl border-2 border-dashed border-primary bg-primary/10 text-sm font-medium text-primary">
          Drop files to attach
        </div>
      </Show>
    </div>
  );
}
