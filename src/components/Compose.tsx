import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import Avatar from "./Avatar";
import Icon from "./Icon";
import Popover from "./ui/Popover";
import { api } from "../lib/api";
import { attachTotal, hasContent, MAX_ATTACH_TOTAL, sameContent, textToHtml } from "../lib/compose";
import { formatSize } from "../lib/format";
import type { Account, ContactSuggestion, Draft, DraftAttachment } from "../lib/types";
import RichEditor, { type EditorApi } from "./RichEditor";

/** Cooldown period after user stops typing before auto-saving to Gmail. */
const COOLDOWN_MS = 2500;
/** Drafts with big files re-upload on every save: save less often. */
const COOLDOWN_BIG_MS = 8000;

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
  /** Demo/screenshot: open already maximized. */
  startMaximized?: boolean;
}

/** Docked compose sheet (bottom right): autosaves to Gmail Drafts. */
export default function Compose(p: Props) {
  const [d, setD] = createSignal<Draft>({ ...p.initial });
  const [account, setAccount] = createSignal<number | null>(p.accountId);
  const [accountMenuOpen, setAccountMenuOpen] = createSignal(false);
  const [showCc, setShowCc] = createSignal(!!(p.initial.cc || p.initial.bcc));
  const [showQuote, setShowQuote] = createSignal(false);
  const [status, setStatus] = createSignal<"" | "editing" | "saving" | "saved" | "error">(p.initial.draftId ? "saved" : "");
  const [error, setError] = createSignal<string | null>(null);
  let saved: Draft | null = p.initial.draftId ? { ...p.initial } : null;
  let pending: Promise<void> | null = null;
  let cooldownTimer: ReturnType<typeof setTimeout> | undefined;
  let blurTimer: ReturnType<typeof setTimeout> | undefined;
  let body: EditorApi | undefined;
  let to: HTMLInputElement | undefined;
  let picker: HTMLInputElement | undefined;
  const [uploading, setUploading] = createSignal(0);
  const [dragging, setDragging] = createSignal(false);
  const [maximized, setMaximized] = createSignal(!!p.startMaximized);

  const schedule = () => {
    setError(null);
    setStatus("editing");
    clearTimeout(cooldownTimer);
    const delay = attachTotal(d().attachments) > 5 * 1024 * 1024 ? COOLDOWN_BIG_MS : COOLDOWN_MS;
    cooldownTimer = setTimeout(() => void save(), delay);
  };
  const set = (k: "to" | "cc" | "bcc" | "subject" | "body", v: string) => {
    setD((x) => ({ ...x, [k]: v }));
    schedule();
  };
  const [activeField, setActiveField] = createSignal<"to" | "cc" | "bcc" | null>(null);
  const [suggestions, setSuggestions] = createSignal<ContactSuggestion[]>([]);
  const [selectedIndex, setSelectedIndex] = createSignal(0);

  const queryContacts = (inputVal: string) => {
    const parts = inputVal.split(",");
    const currentToken = parts[parts.length - 1].trim();
    const cleanToken = currentToken.replace(/^[<"]+|[>"]+$/g, "").trim();
    const curAcct = account();
    const aid = curAcct != null && curAcct > 0 ? curAcct : null;
    void api.searchContacts(cleanToken, aid).then((res) => {
      setSuggestions(res || []);
      setSelectedIndex(0);
    });
  };

  const handleRecipientInput = (field: "to" | "cc" | "bcc", val: string) => {
    set(field, val);
    setActiveField(field);
    queryContacts(val);
  };

  const handleRecipientFocus = (field: "to" | "cc" | "bcc") => {
    clearTimeout(blurTimer);
    setActiveField(field);
    queryContacts(d()[field]);
  };

  const handleRecipientBlur = () => {
    clearTimeout(blurTimer);
    blurTimer = setTimeout(() => {
      setActiveField(null);
      setSuggestions([]);
    }, 250);
  };

  const selectContact = (contact: ContactSuggestion) => {
    const f = activeField();
    if (!f) return;
    const curVal = d()[f];
    const parts = curVal.split(",");
    parts.pop();
    const formatted =
      contact.name && contact.name !== contact.email
        ? `"${contact.name}" <${contact.email}>`
        : contact.email;
    parts.push(parts.length ? ` ${formatted}` : formatted);
    set(f, parts.join(",") + ", ");
    setSuggestions([]);
  };

  const handleRecipientKeyDown = (e: KeyboardEvent) => {
    if (suggestions().length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((i) => Math.min(i + 1, suggestions().length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter" || e.key === "Tab") {
      e.preventDefault();
      const c = suggestions()[selectedIndex()];
      if (c) selectContact(c);
    } else if (e.key === "Escape") {
      setSuggestions([]);
    }
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
    if (saved && sameContent(saved, cur) && cur.draftId) {
      setStatus("saved");
      return true;
    }
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
    clearTimeout(cooldownTimer);
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
    clearTimeout(cooldownTimer);
    const ok = await save();
    p.onClose(d(), account(), ok);
  };

  const discard = async () => {
    clearTimeout(cooldownTimer);
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
      clearTimeout(cooldownTimer);
      clearTimeout(blurTimer);
    });
  });
  let root: HTMLDivElement | undefined;

  const field = "no-ring h-10 w-full min-w-0 bg-transparent text-sm text-text outline-none placeholder:text-faint";
  const currentAccount = () => p.accounts.find((a) => a.id === account()) ?? p.accounts[0];
  const email = () => currentAccount()?.email ?? "";

  const renderSuggestions = () => (
    <div
      class="absolute top-full left-12 z-50 mt-1 max-h-72 w-[380px] max-w-[calc(100vw-48px)] overflow-y-auto rounded-2xl bg-surface-high p-1.5 shadow-2xl border border-divider/60 backdrop-blur-md animate-pop-in"
      role="listbox"
      aria-label="Contact suggestions"
    >
      <div class="flex items-center justify-between px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wider text-faint border-b border-divider/40 mb-1">
        <span>Suggested Contacts</span>
        <span class="text-[9px] font-normal lowercase tracking-normal">↑↓ to navigate · Enter to select</span>
      </div>
      <For each={suggestions()}>
        {(c, idx) => {
          const isSelected = () => idx() === selectedIndex();
          const displayName = () => (c.name && c.name !== c.email ? c.name : c.email);
          const showEmail = () => c.name && c.name !== c.email;
          return (
            <button
              type="button"
              role="option"
              aria-selected={isSelected()}
              class={`flex w-full items-center gap-3 rounded-xl px-2.5 py-2 text-left text-xs transition-colors ${
                isSelected()
                  ? "bg-hover text-primary font-medium shadow-xs"
                  : "text-text hover:bg-hover"
              }`}
              onMouseDown={(e) => {
                e.preventDefault();
                selectContact(c);
              }}
            >
              <Avatar name={displayName()} email={c.email} size={30} />
              <div class="min-w-0 flex-1">
                <div class="truncate font-medium text-text">{displayName()}</div>
                <Show when={showEmail()}>
                  <div class="truncate text-[11px] text-muted">{c.email}</div>
                </Show>
              </div>
              <Show when={c.frequency > 1}>
                <span class="rounded-full bg-surface-dim border border-divider/40 px-2 py-0.5 text-[10px] text-faint shrink-0 font-mono">
                  {c.frequency > 99 ? "99+" : c.frequency}
                </span>
              </Show>
            </button>
          );
        }}
      </For>
    </div>
  );

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
      class={
        maximized()
          ? "fixed inset-4 z-40 flex flex-col rounded-2xl bg-surface shadow-2xl ring-1 ring-black/10 animate-dialog-in dark:ring-white/10"
          : "fixed bottom-0 right-6 z-40 flex h-[min(600px,calc(100vh-88px))] w-[560px] max-w-[calc(100vw-32px)] flex-col overflow-hidden rounded-t-2xl bg-surface shadow-2xl ring-1 ring-black/10 animate-dialog-in dark:ring-white/10"
      }
      role="dialog"
      aria-label={p.title}
    >
      <div class="flex h-11 shrink-0 items-center gap-2 bg-surface-container pl-4 pr-1.5">
        <span class="flex-1 truncate text-sm font-medium text-text" dir="auto">{d().subject || p.title}</span>
        <span class="text-xs text-faint flex items-center gap-1" aria-live="polite">
          {status() === "editing" ? (
            <span class="text-faint">Unsaved changes…</span>
          ) : status() === "saving" ? (
            <span class="text-primary font-medium">Saving draft…</span>
          ) : status() === "saved" ? (
            <span class="text-faint flex items-center gap-1"><Icon name="check" size={14} class="text-otp" /> Draft saved</span>
          ) : status() === "error" ? (
            <span class="text-danger flex items-center gap-1">Not saved <button type="button" class="underline hover:brightness-110 ml-0.5" onClick={() => void save()}>Retry</button></span>
          ) : null}
        </span>
        <button
          class="grid size-8 place-items-center rounded-full text-muted hover:bg-hover"
          title={maximized() ? "Exit full screen" : "Full screen"}
          onClick={() => setMaximized((v) => !v)}
        >
          <Icon name={maximized() ? "fullscreenExit" : "fullscreen"} size={18} />
        </button>
        <button class="grid size-8 place-items-center rounded-full text-muted hover:bg-hover" title="Save & close (Esc)" onClick={() => void close()}>
          <Icon name="close" size={18} />
        </button>
      </div>

      {/* Styled From selector */}
      <div class="relative flex shrink-0 items-center gap-2 border-b border-divider px-4">
        <span class="w-10 text-sm text-faint">From</span>
        <Show
          when={p.canPickAccount && p.accounts.length > 1}
          fallback={
            <div class="flex h-10 flex-1 items-center gap-2 select-none">
              <Avatar
                name={currentAccount()?.displayName || currentAccount()?.email || "?"}
                email={currentAccount()?.email ?? ""}
                size={22}
                color={currentAccount()?.color}
                picture={currentAccount()?.avatarData}
              />
              <span class="truncate text-sm text-text font-medium">{email()}</span>
            </div>
          }
        >
          <div class="relative flex-1">
            <button
              type="button"
              class="flex h-8 items-center gap-2 rounded-lg px-2 text-left text-sm text-text hover:bg-hover transition-colors"
              onClick={() => setAccountMenuOpen((o) => !o)}
            >
              <Avatar
                name={currentAccount()?.displayName || currentAccount()?.email || "?"}
                email={currentAccount()?.email ?? ""}
                size={22}
                color={currentAccount()?.color}
                picture={currentAccount()?.avatarData}
              />
              <span class="truncate font-medium">{email()}</span>
              <Icon name="chevronDown" size={16} class="text-muted" />
            </button>
            <Show when={accountMenuOpen()}>
              <Popover label="Select sending account" align="left" onClose={() => setAccountMenuOpen(false)} class="w-72 p-1.5">
                <div class="px-2.5 py-1 text-[11px] font-semibold uppercase tracking-wider text-faint">
                  Send from account
                </div>
                <For each={p.accounts}>
                  {(a) => {
                    const isSelected = () => (account() ?? p.accounts[0]?.id) === a.id;
                    return (
                      <button
                        type="button"
                        class={`flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-xs transition-colors ${
                          isSelected() ? "bg-hover font-semibold text-text" : "text-text hover:bg-hover"
                        }`}
                        onClick={() => {
                          setAccount(a.id);
                          setD((x) => ({ ...x, draftId: null }));
                          saved = null;
                          setAccountMenuOpen(false);
                          schedule();
                        }}
                      >
                        <Avatar name={a.displayName || a.email} email={a.email} size={26} color={a.color} picture={a.avatarData} />
                        <div class="min-w-0 flex-1">
                          <Show when={a.displayName && a.displayName !== a.email}>
                            <div class="truncate font-medium text-text">{a.displayName}</div>
                          </Show>
                          <div class="truncate text-muted">{a.email}</div>
                        </div>
                        <Show when={isSelected()}>
                          <Icon name="check" size={16} class="text-primary shrink-0" />
                        </Show>
                      </button>
                    );
                  }}
                </For>
              </Popover>
            </Show>
          </div>
        </Show>
      </div>

      {/* To Field with Autocomplete */}
      <div class={`relative flex shrink-0 items-center gap-2 border-b border-divider px-4 ${activeField() === "to" ? "z-40" : "z-20"}`}>
        <span class="w-10 text-sm text-faint">To</span>
        <input
          ref={to}
          class={field}
          value={d().to}
          onInput={(e) => handleRecipientInput("to", e.currentTarget.value)}
          onFocus={() => handleRecipientFocus("to")}
          onBlur={handleRecipientBlur}
          onKeyDown={handleRecipientKeyDown}
          aria-label="To"
          spellcheck={false}
        />
        <Show when={!showCc()}>
          <button class="whitespace-nowrap text-sm text-muted hover:text-text" onClick={() => setShowCc(true)}>
            Cc Bcc
          </button>
        </Show>
        <Show when={activeField() === "to" && suggestions().length > 0}>
          {renderSuggestions()}
        </Show>
      </div>

      {/* Optional Cc / Bcc Fields with Autocomplete */}
      <Show when={showCc()}>
        <div class={`relative flex shrink-0 items-center gap-2 border-b border-divider px-4 ${activeField() === "cc" ? "z-40" : "z-20"}`}>
          <span class="w-10 text-sm text-faint">Cc</span>
          <input
            class={field}
            value={d().cc}
            onInput={(e) => handleRecipientInput("cc", e.currentTarget.value)}
            onFocus={() => handleRecipientFocus("cc")}
            onBlur={handleRecipientBlur}
            onKeyDown={handleRecipientKeyDown}
            aria-label="Cc"
            spellcheck={false}
          />
          <Show when={activeField() === "cc" && suggestions().length > 0}>
            {renderSuggestions()}
          </Show>
        </div>
        <div class={`relative flex shrink-0 items-center gap-2 border-b border-divider px-4 ${activeField() === "bcc" ? "z-40" : "z-20"}`}>
          <span class="w-10 text-sm text-faint">Bcc</span>
          <input
            class={field}
            value={d().bcc}
            onInput={(e) => handleRecipientInput("bcc", e.currentTarget.value)}
            onFocus={() => handleRecipientFocus("bcc")}
            onBlur={handleRecipientBlur}
            onKeyDown={handleRecipientKeyDown}
            aria-label="Bcc"
            spellcheck={false}
          />
          <Show when={activeField() === "bcc" && suggestions().length > 0}>
            {renderSuggestions()}
          </Show>
        </div>
      </Show>

      {/* Subject Field */}
      <div class="flex shrink-0 items-center gap-2 border-b border-divider px-4">
        <input
          class={field}
          placeholder="Subject"
          value={d().subject}
          onInput={(e) => set("subject", e.currentTarget.value)}
          aria-label="Subject"
          dir="auto"
        />
      </div>

      <div class="relative flex min-h-0 flex-1 flex-col">
        <Show when={error()}>
          <div class="flex items-center gap-2 bg-danger/10 px-4 py-2 text-xs text-danger">
            <Icon name="error" size={16} />
            <span class="flex-1">{error()}</span>
            <button class="text-danger hover:underline" onClick={() => setError(null)}>
              Dismiss
            </button>
          </div>
        </Show>

        <div class="flex min-h-0 flex-1 flex-col overflow-y-auto px-4 py-2">
          <RichEditor
            ref={(api) => (body = api)}
            html={d().html || textToHtml(d().body)}
            onChange={setBody}
          />

          <Show when={d().quote}>
            <div class="mt-4 border-t border-divider pt-2">
              <button
                type="button"
                class="rounded-md bg-surface-container px-2 py-0.5 text-xs text-muted hover:text-text"
                title={showQuote() ? "Hide quoted text" : "Show quoted text"}
                onClick={() => setShowQuote((s) => !s)}
              >
                •••
              </button>
              <Show when={showQuote()}>
                <blockquote class="selectable mt-2 whitespace-pre-wrap border-l-2 border-divider pl-3 text-xs leading-5 text-muted" dir="auto">
                  {d().quote!.header}
                  {"\n"}
                  {d().quote!.text}
                </blockquote>
              </Show>
            </div>
          </Show>

          <Show when={d().attachments.length > 0}>
            <div class="mt-3 flex flex-wrap gap-2 border-t border-divider pt-2">
              <For each={d().attachments}>
                {(a, i) => (
                  <div class="flex items-center gap-2 rounded-xl bg-surface-container px-3 py-1.5 text-xs text-text ring-1 ring-divider">
                    <Icon name="attachment" size={16} class="text-muted" />
                    <span class="max-w-[180px] truncate">{a.name}</span>
                    <span class="text-faint">{formatSize(a.size)}</span>
                    <button
                      class="grid size-5 place-items-center rounded-full text-muted hover:bg-hover hover:text-text"
                      title="Remove"
                      onClick={() => removeFile(i())}
                    >
                      <Icon name="close" size={14} />
                    </button>
                  </div>
                )}
              </For>
            </div>
          </Show>
        </div>

        <Show when={dragging()}>
          <div class="pointer-events-none absolute inset-0 z-30 grid place-items-center rounded-b-2xl bg-primary/10 backdrop-blur-xs">
            <div class="flex flex-col items-center gap-2 text-primary">
              <Icon name="attachment" size={32} />
              <span class="text-sm font-medium">Drop files here to attach</span>
            </div>
          </div>
        </Show>
      </div>

      <div class="flex shrink-0 items-center gap-2 border-t border-divider bg-surface-container px-4 py-2.5">
        <button
          class="flex h-9 items-center gap-2 rounded-full bg-primary px-5 text-sm font-medium text-on-primary shadow-xs hover:shadow transition-shadow disabled:opacity-50"
          disabled={uploading() > 0}
          onClick={() => void send()}
        >
          <span>Send</span>
          <Icon name="send" size={16} />
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
        <button
          class="grid size-9 place-items-center rounded-full text-muted hover:bg-hover hover:text-text"
          title="Attach files (up to 25 MB)"
          onClick={() => picker?.click()}
        >
          <Icon name="attachment" size={18} />
        </button>
        <Show when={uploading() > 0}>
          <span class="text-xs text-muted">Uploading attachments…</span>
        </Show>

        <button
          class="ml-auto grid size-9 place-items-center rounded-full text-muted hover:bg-hover hover:text-danger"
          title="Discard draft"
          onClick={() => void discard()}
        >
          <Icon name="trash" size={18} />
        </button>
      </div>
    </div>
  );
}
