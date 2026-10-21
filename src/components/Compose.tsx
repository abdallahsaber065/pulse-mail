import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import Icon from "./Icon";
import { api } from "../lib/api";
import { hasContent, sameContent } from "../lib/compose";
import type { Account, Draft } from "../lib/types";

const AUTOSAVE_MS = 2000;

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
  let body: HTMLTextAreaElement | undefined;
  let to: HTMLInputElement | undefined;

  const set = (k: keyof Draft, v: string) => {
    setD((x) => ({ ...x, [k]: v }));
    setError(null);
    clearTimeout(timer);
    timer = setTimeout(() => void save(), AUTOSAVE_MS);
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
    (p.initial.to ? body : to)?.focus();
    if (p.initial.to) body?.setSelectionRange(0, 0);
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
        <textarea
          ref={body}
          class="no-ring min-h-[160px] w-full flex-1 resize-none bg-transparent text-sm leading-6 text-text outline-none"
          value={d().body}
          onInput={(e) => set("body", e.currentTarget.value)}
          aria-label="Message"
          dir="auto"
        />
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
                  set("body", d().body);
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
        <button class="ml-auto grid size-9 place-items-center rounded-full text-muted hover:bg-hover" title="Discard draft" onClick={() => void discard()}>
          <Icon name="trash" size={18} />
        </button>
      </div>
    </div>
  );
}
