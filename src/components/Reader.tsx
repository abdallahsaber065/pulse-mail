import { createEffect, createSignal, For, Index, Show } from "solid-js";
import { api } from "../lib/api";
import { formatSize } from "../lib/format";
import Icon, { type IconName } from "./Icon";
import Avatar from "./Avatar";
import EmailFrame from "./EmailFrame";
import AttachmentPreview from "./AttachmentPreview";
import Popover from "./ui/Popover";
import { fullDate, labelName } from "../lib/format";
import { theme } from "../lib/theme";
import type { Attachment, Message, RenderedMessage, Category } from "../lib/types";

interface Props {
  messages: Message[];
  /** Account the thread belongs to (null = default account). */
  accountId: number | null;
  /** Thread clicked but nothing cached for it yet. */
  loading: boolean;
  /** Bodies couldn't be downloaded; show an inline retry instead of a skeleton. */
  error: string | null;
  onRetry(): void;
  onBack(): void;
  onCopyCode(code: string): void;
  /** "Not a code" was pressed (the code is cleared; reload the thread). */
  onOtpChanged?(): void;
  onArchive?(): void;
  onTrash?(): void;
  onSpam?(): void;
  onToggleRead(): void;
  onToggleStar(): void;
  onToggleImportant?(): void;
  /** A link inside an email was clicked. */
  onLink(href: string, text: string): void;
  onAttachment(m: Message, a: Attachment, open: boolean): void;
  /** Dark-theme email colors: smart invert or the sender's own. */
  emailDark: "smart" | "original";
  onToggleEmailDark(): void | Promise<void>;
  /** Reply / reply all / forward a message. */
  onCompose(m: Message, mode: "reply" | "reply_all" | "forward"): void;
  /** Edit a draft message. */
  onEditDraft?(m: Message): void;
  /** Move conversation to a category tab. */
  onMoveCategory?(c: Category): void;
  /** Quick reply directly from the reader. */
  onQuickSend?(m: Message, text: string, mode: "reply" | "reply_all"): Promise<void> | void;
}

const SYSTEM_INTERNAL = new Set(["UNREAD", "STARRED", "IMPORTANT", "SENT", "INBOX", "CHAT"]);
const CATEGORY_CONFIG: Record<Category, { name: string; icon: IconName; desc: string }> = {
  CATEGORY_PERSONAL: { name: "Primary", icon: "inbox", desc: "Personal and direct conversations" },
  CATEGORY_PROMOTIONS: { name: "Promotions", icon: "label", desc: "Offers, deals, and marketing" },
  CATEGORY_SOCIAL: { name: "Social", icon: "person", desc: "Social networks and media" },
  CATEGORY_UPDATES: { name: "Updates", icon: "info", desc: "Receipts, bills, and notifications" },
  CATEGORY_FORUMS: { name: "Forums", icon: "mail", desc: "Discussions and group messages" },
};
const CATEGORY_NAMES: Record<string, string> = Object.fromEntries(
  Object.entries(CATEGORY_CONFIG).map(([k, v]) => [k, v.name]),
);

function BodySkeleton() {
  return (
    <div class="space-y-2 py-2" aria-label="Loading message">
      <div class="skeleton h-3 w-11/12 rounded-full" />
      <div class="skeleton h-3 w-10/12 rounded-full" style={{ "animation-delay": "80ms" }} />
      <div class="skeleton h-3 w-9/12 rounded-full" style={{ "animation-delay": "160ms" }} />
      <div class="skeleton h-3 w-7/12 rounded-full" style={{ "animation-delay": "240ms" }} />
    </div>
  );
}

export default function Reader(p: Props) {
  const [imagesFor, setImagesFor] = createSignal<Set<string>>(new Set());
  const [info, setInfo] = createSignal<Record<string, RenderedMessage>>({});
  const [version, setVersion] = createSignal(0);
  const [copied, setCopied] = createSignal(false);
  const [catMenuOpen, setCatMenuOpen] = createSignal(false);
  const [quickReplyOpen, setQuickReplyOpen] = createSignal(false);
  const [quickReplyText, setQuickReplyText] = createSignal("");
  const [quickReplyMode, setQuickReplyMode] = createSignal<"reply" | "reply_all">("reply");
  const [quickSending, setQuickSending] = createSignal(false);
  const [preview, setPreview] = createSignal<{ message: Message; attachment: Attachment } | null>(null);

  createEffect(() => {
    const params = typeof window !== "undefined" ? new URLSearchParams(window.location.search) : null;
    if (params?.has("preview") && !preview()) {
      const ms = p.messages;
      const target = ms.find((m) => m.attachments.length > 0);
      if (target && target.attachments[0]) {
        setPreview({ message: target, attachment: target.attachments[0] });
      }
    }
  });
  let replyTextarea: HTMLTextAreaElement | undefined;
  const lastMsg = () => p.messages[p.messages.length - 1];

  const handleQuickSend = async () => {
    const text = quickReplyText().trim();
    const last = lastMsg();
    if (!text || !last || quickSending()) return;
    setQuickSending(true);
    try {
      if (p.onQuickSend) {
        await p.onQuickSend(last, text, quickReplyMode());
      } else {
        p.onCompose(last, quickReplyMode());
      }
      setQuickReplyText("");
      setQuickReplyOpen(false);
    } finally {
      setQuickSending(false);
    }
  };

  const starred = () => p.messages.some((m) => m.starred);
  const unread = () => p.messages.some((m) => m.unread);
  const important = () => p.messages.some((m) => m.labelIds.includes("IMPORTANT"));
  const otpMsg = () => [...p.messages].reverse().find((m) => m.otpCode);
  // Sign-in links expire quickly; only offer them on recent mail.
  const linkMsg = () =>
    otpMsg() ? undefined : [...p.messages].reverse().find((m) => m.otpLink && Date.now() - m.internalDate < 3 * 86_400_000);
  const feedback = (useful: boolean) => {
    const m = otpMsg() ?? linkMsg();
    if (!m || p.accountId == null) return;
    void api
      .otpFeedback(p.accountId, m.fromEmail, useful, useful ? null : m.id)
      .then(() => !useful && p.onOtpChanged?.())
      .catch((e) => console.warn(e));
  };
  const copy = (code: string) => {
    p.onCopyCode(code);
    feedback(true);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1600);
  };
  const Btn = (b: { icon: IconName; title: string; onClick(): void; active?: boolean }) => (
    <button
      class={`grid size-10 place-items-center rounded-full hover:bg-hover ${b.active ? "text-star" : "text-muted"}`}
      onClick={b.onClick}
      title={b.title}
      aria-label={b.title}
    >
      <Icon name={b.icon} />
    </button>
  );
  const first = () => p.messages[0];
  const otp = () => otpMsg()?.otpCode ?? null;
  const showImages = (id: string) => setImagesFor((s) => new Set(s).add(id));
  const alwaysShow = async (m: Message) => {
    try {
      await api.setImageSender(m.fromEmail, true, p.accountId);
      showImages(m.id);
      setVersion((v) => v + 1);
    } catch (e) {
      // surfaced by the caller's snackbar elsewhere; keep the bar
      console.warn(e);
    }
  };
  const fileIcon = (a: Attachment): IconName =>
    a.mime.startsWith("image/") ? "image" : a.mime === "application/pdf" ? "pdf" : "attachment";

  const [togglingDark, setTogglingDark] = createSignal(false);
  const handleToggleEmailDark = async () => {
    if (togglingDark()) return;
    setTogglingDark(true);
    try {
      await p.onToggleEmailDark();
      setVersion((v) => v + 1);
    } finally {
      setTogglingDark(false);
    }
  };

  return (
    <Show
      when={first()}
      fallback={
        <Show when={p.loading} fallback={<div class="grid h-full place-items-center text-sm text-muted">Select a conversation to read it.</div>}>
          <div class="px-20 pt-16">
            <div class="skeleton mb-6 h-6 w-1/2 rounded-full" />
            <BodySkeleton />
          </div>
        </Show>
      }
    >
      <div class="reader h-full overflow-y-auto">
        <div class="no-print sticky top-0 z-10 flex h-12 items-center gap-1 border-b border-divider bg-surface px-2">
          <Btn icon="back" title="Back to list (u)" onClick={p.onBack} />
          <div class="mx-1 h-5 w-px bg-divider" />
          <Show when={p.onArchive}>
            <Btn icon="archive" title="Archive (e)" onClick={() => p.onArchive!()} />
          </Show>
          <Show when={p.onTrash}>
            <Btn icon="trash" title="Move to trash (#)" onClick={() => p.onTrash!()} />
          </Show>
          <Show when={p.onSpam}>
            <Btn icon="spam" title="Report spam (!)" onClick={() => p.onSpam!()} />
          </Show>
          <Btn
            icon={unread() ? "markRead" : "markUnread"}
            title={unread() ? "Mark as read (Shift+I)" : "Mark as unread (Shift+U)"}
            onClick={p.onToggleRead}
          />
          <Btn icon={starred() ? "starFilled" : "star"} title={starred() ? "Unstar (s)" : "Star (s)"} onClick={p.onToggleStar} active={starred()} />
          <Show when={p.onToggleImportant}>
            <Btn
              icon="important"
              title={important() ? "Mark as not important" : "Mark as important"}
              onClick={p.onToggleImportant!}
              active={important()}
            />
          </Show>
          <Show when={p.onMoveCategory}>
            <div class="mx-1 h-5 w-px bg-divider" />
            <div class="relative">
              {(() => {
                const currentCat = () =>
                  (first().labelIds.find((l) => l in CATEGORY_CONFIG) as Category | undefined) ?? "CATEGORY_PERSONAL";
                return (
                  <>
                    <button
                      class="flex h-7 items-center gap-1.5 rounded-full bg-surface-container px-2.5 text-xs font-medium text-text transition-colors hover:bg-hover hover:ring-1 hover:ring-outline/20"
                      onClick={() => setCatMenuOpen((o) => !o)}
                      title="Move conversation to category tab"
                      aria-haspopup="true"
                      aria-expanded={catMenuOpen()}
                    >
                      <Icon name={CATEGORY_CONFIG[currentCat()].icon} size={14} class="text-muted" />
                      <span>{CATEGORY_CONFIG[currentCat()].name}</span>
                      <Icon
                        name="chevronDown"
                        size={12}
                        class={`text-muted transition-transform duration-200 ${catMenuOpen() ? "rotate-180" : ""}`}
                      />
                    </button>
                    <Show when={catMenuOpen()}>
                      <Popover onClose={() => setCatMenuOpen(false)} align="left" label="Select category tab" class="w-64 p-1.5 shadow-xl">
                        <div class="px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wider text-faint">
                          Move to Tab
                        </div>
                        <For each={Object.entries(CATEGORY_CONFIG) as [Category, { name: string; icon: IconName; desc: string }][]}>
                          {([catId, cfg]) => {
                            const isCurrent = () => currentCat() === catId;
                            return (
                              <button
                                class={`flex w-full items-start gap-2.5 rounded-xl px-2.5 py-2 text-left transition-colors ${
                                  isCurrent() ? "bg-selected text-primary font-medium" : "text-text hover:bg-hover"
                                }`}
                                onClick={() => {
                                  setCatMenuOpen(false);
                                  p.onMoveCategory?.(catId);
                                }}
                              >
                                <div class="mt-0.5 grid size-6 shrink-0 place-items-center rounded-lg bg-surface-dim">
                                  <Icon name={cfg.icon} size={14} class={isCurrent() ? "text-primary" : "text-muted"} />
                                </div>
                                <div class="flex-1 min-w-0">
                                  <div class="flex items-center justify-between">
                                    <span class="text-xs font-semibold">{cfg.name}</span>
                                    <Show when={isCurrent()}>
                                      <Icon name="check" size={14} class="text-primary" />
                                    </Show>
                                  </div>
                                  <div class="text-[11px] text-faint truncate">{cfg.desc}</div>
                                </div>
                              </button>
                            );
                          }}
                        </For>
                      </Popover>
                    </Show>
                  </>
                );
              })()}
            </div>
          </Show>
          <div class="ml-auto flex items-center gap-1">
            <span class="pr-2 text-xs text-faint">{p.messages.length > 1 ? `${p.messages.length} messages` : ""}</span>
            <Show when={theme.isDark()}>
              <Btn
                icon={p.emailDark === "smart" ? "contrast" : "lightMode"}
                title={p.emailDark === "smart" ? "Show emails in their original colors" : "Show emails in dark colors"}
                onClick={handleToggleEmailDark}
              />
            </Show>
            <Btn icon="print" title="Print" onClick={() => window.print()} />
          </div>
        </div>

        <div class="px-6 pb-10 pt-4">
          <div class="mb-4 flex flex-wrap items-center gap-2 pl-14">
            <h1 class="selectable font-display text-[22px] leading-tight text-text" dir="auto">
              {first().subject || "(no subject)"}
            </h1>
            <For each={first().labelIds.filter((l) => !SYSTEM_INTERNAL.has(l))}>
              {(l) => {
                const isCat = l in CATEGORY_NAMES;
                return (
                  <span
                    class={`inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium ${
                      isCat
                        ? "bg-secondary-container text-on-secondary-container font-semibold"
                        : "bg-surface-container text-muted border border-outline-variant"
                    }`}
                  >
                    <Show when={isCat}><Icon name="label" size={12} /></Show>
                    {isCat ? CATEGORY_NAMES[l] : labelName(l, l)}
                  </span>
                );
              }}
            </For>
          </div>

          <Show when={otp()}>
            <div class="mb-5 ml-14 flex max-w-xl items-center gap-4 rounded-2xl bg-otp-container px-5 py-3 text-otp">
              <div class="flex-1">
                <div class="text-xs font-medium uppercase tracking-wide opacity-80">Verification code</div>
                <div class="selectable font-mono text-[28px] font-semibold tracking-[0.2em]">{otp()}</div>
              </div>
              <button
                class="no-print flex items-center gap-2 rounded-full bg-otp px-4 py-2 text-sm font-medium text-surface hover:brightness-110"
                onClick={() => copy(otp()!)}
                title="Copy code (Shift+C)"
              >
                <Icon name={copied() ? "checkCircle" : "copy"} size={18} /> {copied() ? "Copied" : "Copy"}
              </button>
              <button
                class="no-print rounded-full px-3 py-2 text-xs font-medium opacity-80 hover:bg-black/5 hover:opacity-100 dark:hover:bg-white/10"
                onClick={() => feedback(false)}
                title="Not a verification code: hide it and trust this sender's codes less"
              >
                Not a code
              </button>
            </div>
          </Show>
          <Show when={linkMsg()}>
            {(m) => (
              <div class="mb-5 ml-14 flex max-w-xl items-center gap-4 rounded-2xl bg-otp-container px-5 py-3 text-otp">
                <div class="min-w-0 flex-1">
                  <div class="text-xs font-medium uppercase tracking-wide opacity-80">Sign-in link</div>
                  <div class="truncate text-sm opacity-80" title={m().otpLink!}>
                    {(() => {
                      try {
                        return new URL(m().otpLink!).hostname;
                      } catch {
                        return m().otpLink;
                      }
                    })()}
                  </div>
                </div>
                <button
                  class="no-print flex items-center gap-2 rounded-full bg-otp px-4 py-2 text-sm font-medium text-surface hover:brightness-110"
                  onClick={() => p.onLink(m().otpLink!, "Sign-in link")}
                >
                  <Icon name="openExternal" size={18} /> Open
                </button>
                <button
                  class="no-print rounded-full px-3 py-2 text-xs font-medium opacity-80 hover:bg-black/5 hover:opacity-100 dark:hover:bg-white/10"
                  onClick={() => feedback(false)}
                  title="Not a sign-in link: hide it"
                >
                  Not a link
                </button>
              </div>
            )}
          </Show>

          <Index each={p.messages}>
            {(m) => {
              const r = () => info()[m().id];
              const hidden = () => (r()?.remoteImages ?? 0) > 0 && (r()?.imagesShown ?? 0) === 0 && !imagesFor().has(m().id);
              return (
                <article class="mb-6">
                  <div class="mb-3 flex items-start gap-3">
                    <Avatar name={m().fromName} email={m().fromEmail} />
                    <div class="min-w-0 flex-1">
                      <div class="flex items-baseline gap-2">
                        <span class="selectable truncate font-semibold text-text" dir="auto">{m().fromName || m().fromEmail}</span>
                        <span class="selectable truncate text-xs text-muted">&lt;{m().fromEmail}&gt;</span>
                        <span class="ml-auto shrink-0 text-xs text-muted">{fullDate(m().internalDate)}</span>
                        <span class="no-print -my-2 flex shrink-0 items-center">
                          <Btn icon="reply" title="Reply (r)" onClick={() => p.onCompose(m(), "reply")} />
                          <Btn icon="forward" title="Forward (f)" onClick={() => p.onCompose(m(), "forward")} />
                        </span>
                      </div>
                      <div class="truncate text-xs text-muted">
                        to {m().toList || "me"}
                        <Show when={m().ccList}>, cc {m().ccList}</Show>
                      </div>
                    </div>
                  </div>
                  <div class="no-print ml-14 mb-3">
                    <Show when={hidden() || (r()?.trackers ?? 0) > 0}>
                      <div class="flex flex-wrap items-center justify-between gap-3 rounded-xl bg-surface-container/80 border border-divider/40 px-3.5 py-2 text-xs text-muted">
                        <div class="flex items-center gap-2.5 min-w-0">
                          <Show when={hidden()} fallback={<Icon name="shield" size={15} class="text-secondary shrink-0" />}>
                            <Icon name="image" size={15} class="text-muted shrink-0" />
                          </Show>
                          <span class="truncate">
                            {hidden()
                              ? "Remote images hidden to protect privacy"
                              : `${r()!.trackers === 1 ? "1 tracking pixel" : `${r()!.trackers} tracking pixels`} blocked`}
                          </span>
                        </div>
                        <div class="flex items-center gap-3 shrink-0">
                          <Show when={hidden()}>
                            <button class="font-medium text-primary hover:underline" onClick={() => showImages(m().id)}>
                              Show images
                            </button>
                            <Show when={m().fromEmail}>
                              <button class="hidden sm:inline font-medium text-primary/80 hover:text-primary hover:underline" onClick={() => void alwaysShow(m())}>
                                Always show
                              </button>
                            </Show>
                          </Show>
                          <Show when={(r()?.trackers ?? 0) > 0 && hidden()}>
                            <span
                              class="inline-flex items-center gap-1.5 rounded-full bg-secondary-container/80 px-2.5 py-0.5 text-[11px] font-medium text-on-secondary-container"
                              title="Tracking pixels tell the sender when you open a message. Pulse Mail never loads them."
                            >
                              <Icon name="shield" size={12} />
                              {r()!.trackers === 1 ? "Blocked a tracker" : `Blocked ${r()!.trackers} trackers`}
                            </span>
                          </Show>
                        </div>
                      </div>
                    </Show>
                  </div>
                  <div class="pl-14">
                    <Show
                      when={m().bodyLoaded}
                      fallback={
                        <>
                          <p class="selectable mb-2 text-sm text-muted" dir="auto">{m().snippet}</p>
                          <Show when={p.error} fallback={<BodySkeleton />}>
                            <div class="flex items-center gap-3 rounded-xl bg-danger-container px-4 py-3 text-sm text-on-danger-container" role="alert">
                              <Icon name="error" size={18} />
                              <span class="flex-1">Couldn't load this message. {p.error}</span>
                              <button class="rounded-full px-3 py-1 font-medium text-primary hover:bg-hover" onClick={p.onRetry}>
                                Retry
                              </button>
                            </div>
                          </Show>
                        </>
                      }
                    >
                      <EmailFrame
                        messageId={m().id}
                        accountId={p.accountId}
                        dark={theme.isDark()}
                        showImages={imagesFor().has(m().id)}
                        version={version() + (p.emailDark === "smart" ? 0 : 100000)}
                        onRendered={(x) => setInfo((o) => ({ ...o, [m().id]: x }))}
                        onLink={p.onLink}
                      />
                    </Show>
                    <Show when={m().attachments.length > 0}>
                      <div class="mt-4 flex flex-wrap gap-2" aria-label="Attachments">
                        <For each={m().attachments}>
                          {(a) => (
                            <div
                              class="group flex w-60 items-center gap-2 rounded-xl border border-divider bg-surface-dim px-3 py-2 text-sm cursor-pointer hover:border-primary/50 hover:bg-surface hover:shadow-xs transition-all"
                              title={`Preview ${a.filename}`}
                              onClick={() => setPreview({ message: m(), attachment: a })}
                            >
                              <Icon name={fileIcon(a)} size={20} class="shrink-0 text-muted group-hover:text-primary transition-colors" />
                              <div class="min-w-0 flex-1">
                                <div class="selectable truncate font-medium text-text group-hover:text-primary transition-colors" title={a.filename} dir="auto">{a.filename}</div>
                                <div class="text-xs text-faint">{formatSize(a.size)}</div>
                              </div>
                              <button
                                class="no-print grid size-8 place-items-center rounded-full text-muted hover:bg-hover hover:text-text transition-colors"
                                title="Open in system default app"
                                aria-label={`Open ${a.filename}`}
                                onClick={(e) => {
                                  e.stopPropagation();
                                  p.onAttachment(m(), a, true);
                                }}
                              >
                                <Icon name="openExternal" size={17} />
                              </button>
                              <button
                                class="no-print grid size-8 place-items-center rounded-full text-muted hover:bg-hover hover:text-text transition-colors"
                                title="Save to Downloads"
                                aria-label={`Save ${a.filename}`}
                                onClick={(e) => {
                                  e.stopPropagation();
                                  p.onAttachment(m(), a, false);
                                }}
                              >
                                <Icon name="download" size={17} />
                              </button>
                            </div>
                          )}
                        </For>
                      </div>
                    </Show>
                  </div>
                </article>
              );
            }}
          </Index>
          {/* Inline Quick Reply Card */}
          <Show when={lastMsg() && !lastMsg().labelIds.includes("DRAFT")}>
            <div class="no-print ml-14 mt-4">
              <Show
                when={quickReplyOpen()}
                fallback={
                  <div class="flex flex-wrap items-center gap-2">
                    <button
                      class="flex h-10 flex-1 min-w-[260px] items-center gap-3 rounded-2xl border border-divider bg-surface-container/60 px-4 text-left text-sm text-muted transition-all hover:border-outline/40 hover:bg-surface-container hover:text-text hover:shadow-sm"
                      onClick={() => {
                        setQuickReplyOpen(true);
                        setTimeout(() => replyTextarea?.focus(), 50);
                      }}
                    >
                      <Icon name="reply" size={16} class="text-muted" />
                      <span>Reply to {lastMsg()?.fromName || lastMsg()?.fromEmail || "conversation"}…</span>
                    </button>
                    <button
                      class="flex h-10 items-center gap-2 rounded-full border border-divider px-4 text-xs font-medium text-muted hover:bg-hover hover:text-text transition-colors"
                      onClick={() => p.onCompose(lastMsg()!, "reply_all")}
                      title="Reply to all recipients"
                    >
                      <Icon name="replyAll" size={16} /> Reply all
                    </button>
                    <button
                      class="flex h-10 items-center gap-2 rounded-full border border-divider px-4 text-xs font-medium text-muted hover:bg-hover hover:text-text transition-colors"
                      onClick={() => p.onCompose(lastMsg()!, "forward")}
                      title="Forward conversation"
                    >
                      <Icon name="forward" size={16} /> Forward
                    </button>
                  </div>
                }
              >
                <div class="rounded-2xl border border-primary/30 bg-surface shadow-lg ring-1 ring-primary/20 transition-all p-3.5 animate-scale-in">
                  <div class="mb-2 flex items-center justify-between text-xs text-muted">
                    <div class="flex items-center gap-2">
                      <select
                        class="no-ring rounded-lg bg-surface-container px-2 py-1 text-xs font-medium text-text outline-none cursor-pointer hover:bg-hover"
                        value={quickReplyMode()}
                        onChange={(e) => setQuickReplyMode(e.currentTarget.value as "reply" | "reply_all")}
                      >
                        <option value="reply">Reply</option>
                        <option value="reply_all">Reply all</option>
                      </select>
                      <span class="truncate">to <strong class="text-text">{lastMsg()?.fromName || lastMsg()?.fromEmail}</strong></span>
                    </div>
                    <button
                      class="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-faint hover:bg-hover hover:text-text transition-colors"
                      onClick={() => {
                        p.onCompose(lastMsg()!, quickReplyMode());
                        setQuickReplyOpen(false);
                      }}
                      title="Pop out to full compose window"
                    >
                      <Icon name="openExternal" size={14} /> Pop out
                    </button>
                  </div>
                  <textarea
                    ref={replyTextarea}
                    class="w-full resize-none bg-transparent text-sm text-text outline-none placeholder:text-faint min-h-[90px]"
                    placeholder={`Reply to ${lastMsg()?.fromName || lastMsg()?.fromEmail}… (Ctrl+Enter to send)`}
                    value={quickReplyText()}
                    onInput={(e) => setQuickReplyText(e.currentTarget.value)}
                    onKeyDown={(e) => {
                      if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
                        e.preventDefault();
                        void handleQuickSend();
                      }
                      if (e.key === "Escape" && !quickReplyText().trim()) {
                        setQuickReplyOpen(false);
                      }
                    }}
                  />
                  <div class="mt-2 flex items-center justify-between border-t border-divider pt-2.5">
                    <div class="flex items-center gap-1 text-xs text-faint">
                      <span>Press <kbd class="font-mono text-[10px] bg-surface-container px-1.5 py-0.5 rounded">Ctrl+Enter</kbd> to send</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <button
                        class="rounded-full px-3 py-1.5 text-xs text-muted hover:bg-hover transition-colors"
                        onClick={() => {
                          setQuickReplyText("");
                          setQuickReplyOpen(false);
                        }}
                      >
                        Discard
                      </button>
                      <button
                        class="flex items-center gap-1.5 rounded-full bg-primary px-4 py-1.5 text-xs font-semibold text-on-primary hover:shadow transition-all disabled:opacity-50"
                        disabled={!quickReplyText().trim() || quickSending()}
                        onClick={() => void handleQuickSend()}
                      >
                        <Icon name="send" size={14} />
                        <span>{quickSending() ? "Sending…" : "Send"}</span>
                      </button>
                    </div>
                  </div>
                </div>
              </Show>
            </div>
          </Show>
          <Show when={p.messages.find((m) => m.labelIds.includes("DRAFT") || m.id.startsWith("draft_"))}>
            {(draft) => (
              <div class="no-print ml-14 mt-4">
                <button
                  class="flex h-10 items-center gap-2 rounded-full bg-primary px-5 text-sm font-medium text-on-primary hover:shadow"
                  onClick={() => p.onEditDraft?.(draft())}
                >
                  <Icon name="pencil" size={18} /> Edit draft
                </button>
              </div>
            )}
          </Show>
        </div>
      </div>
      <Show when={preview()}>
        {(pr) => (
          <AttachmentPreview
            messageId={pr().message.id}
            attachment={pr().attachment}
            accountId={p.accountId}
            onClose={() => setPreview(null)}
            onOpenApp={() => p.onAttachment(pr().message, pr().attachment, true)}
            onDownload={() => p.onAttachment(pr().message, pr().attachment, false)}
          />
        )}
      </Show>
    </Show>
  );
}
