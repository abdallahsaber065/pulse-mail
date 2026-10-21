import { createSignal, For, Show } from "solid-js";
import { api } from "../lib/api";
import { formatSize } from "../lib/format";
import Icon, { type IconName } from "./Icon";
import Avatar from "./Avatar";
import EmailFrame from "./EmailFrame";
import { fullDate, labelName } from "../lib/format";
import { theme } from "../lib/theme";
import type { Attachment, Message, RenderedMessage } from "../lib/types";

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
  onArchive?(): void;
  onToggleRead(): void;
  onToggleStar(): void;
  /** A link inside an email was clicked. */
  onLink(href: string, text: string): void;
  onAttachment(m: Message, a: Attachment, open: boolean): void;
  /** Dark-theme email colors: smart invert or the sender's own. */
  emailDark: "smart" | "original";
  onToggleEmailDark(): void;
}

const HIDDEN_LABELS = new Set(["UNREAD", "STARRED", "CATEGORY_PERSONAL", "CATEGORY_UPDATES", "CATEGORY_PROMOTIONS", "CATEGORY_SOCIAL", "CATEGORY_FORUMS", "IMPORTANT", "SENT"]);

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
  const starred = () => p.messages.some((m) => m.starred);
  const unread = () => p.messages.some((m) => m.unread);
  const copy = (code: string) => {
    p.onCopyCode(code);
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
  const otp = () => [...p.messages].reverse().find((m) => m.otpCode)?.otpCode ?? null;
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
      <div class="reader h-full overflow-y-auto animate-fade-in">
        <div class="no-print sticky top-0 z-10 flex h-12 items-center gap-1 border-b border-divider bg-surface px-2">
          <Btn icon="back" title="Back to list (u)" onClick={p.onBack} />
          <div class="mx-1 h-5 w-px bg-divider" />
          <Show when={p.onArchive}>
            <Btn icon="archive" title="Archive (e)" onClick={() => p.onArchive!()} />
          </Show>
          <Btn
            icon={unread() ? "markRead" : "markUnread"}
            title={unread() ? "Mark as read (Shift+I)" : "Mark as unread (Shift+U)"}
            onClick={p.onToggleRead}
          />
          <Btn icon={starred() ? "starFilled" : "star"} title={starred() ? "Unstar (s)" : "Star (s)"} onClick={p.onToggleStar} active={starred()} />
          <div class="ml-auto flex items-center gap-1">
            <span class="pr-2 text-xs text-faint">{p.messages.length > 1 ? `${p.messages.length} messages` : ""}</span>
            <Show when={theme.isDark()}>
              <Btn
                icon={p.emailDark === "smart" ? "contrast" : "lightMode"}
                title={p.emailDark === "smart" ? "Show emails in their original colors" : "Show emails in dark colors"}
                onClick={p.onToggleEmailDark}
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
            <For each={first().labelIds.filter((l) => !HIDDEN_LABELS.has(l))}>
              {(l) => <span class="rounded bg-surface-container px-1.5 py-0.5 text-xs text-muted">{labelName(l, l)}</span>}
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
            </div>
          </Show>

          <For each={p.messages}>
            {(m) => {
              const r = () => info()[m.id];
              const hidden = () => (r()?.remoteImages ?? 0) > 0 && (r()?.imagesShown ?? 0) === 0 && !imagesFor().has(m.id);
              return (
                <article class="mb-6">
                  <div class="mb-3 flex items-start gap-3">
                    <Avatar name={m.fromName} email={m.fromEmail} />
                    <div class="min-w-0 flex-1">
                      <div class="flex items-baseline gap-2">
                        <span class="selectable truncate font-semibold text-text" dir="auto">{m.fromName || m.fromEmail}</span>
                        <span class="selectable truncate text-xs text-muted">&lt;{m.fromEmail}&gt;</span>
                        <span class="ml-auto shrink-0 text-xs text-muted">{fullDate(m.internalDate)}</span>
                      </div>
                      <div class="truncate text-xs text-muted">
                        to {m.toList || "me"}
                        <Show when={m.ccList}>, cc {m.ccList}</Show>
                      </div>
                    </div>
                  </div>
                  <div class="no-print ml-14 flex flex-wrap gap-2">
                    <Show when={hidden()}>
                      <div class="mb-2 flex flex-1 items-center gap-3 rounded-lg bg-surface-container px-3 py-2 text-xs text-muted">
                        <Icon name="image" size={16} />
                        <span class="flex-1">Images are hidden to protect your privacy.</span>
                        <button class="font-medium text-primary hover:underline" onClick={() => showImages(m.id)}>
                          Show images
                        </button>
                        <Show when={m.fromEmail}>
                          <button class="font-medium text-primary hover:underline" onClick={() => void alwaysShow(m)}>
                            Always show from {m.fromEmail}
                          </button>
                        </Show>
                      </div>
                    </Show>
                    <Show when={(r()?.trackers ?? 0) > 0}>
                      <div
                        class="mb-2 flex items-center gap-1.5 self-start rounded-full bg-secondary-container px-2.5 py-1 text-xs text-on-secondary-container"
                        title="Tracking pixels tell the sender when you open a message. Pulse Mail never loads them."
                      >
                        <Icon name="shield" size={14} />
                        {r()!.trackers === 1 ? "Blocked a tracker" : `Blocked ${r()!.trackers} trackers`}
                      </div>
                    </Show>
                  </div>
                  <div class="pl-14">
                    <Show
                      when={m.bodyLoaded}
                      fallback={
                        <>
                          <p class="selectable mb-2 text-sm text-muted" dir="auto">{m.snippet}</p>
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
                        messageId={m.id}
                        accountId={p.accountId}
                        dark={theme.isDark()}
                        showImages={imagesFor().has(m.id)}
                        version={version() + (p.emailDark === "smart" ? 0 : 100000)}
                        onRendered={(x) => setInfo((o) => ({ ...o, [m.id]: x }))}
                        onLink={p.onLink}
                      />
                    </Show>
                    <Show when={m.attachments.length > 0}>
                      <div class="mt-4 flex flex-wrap gap-2" aria-label="Attachments">
                        <For each={m.attachments}>
                          {(a) => (
                            <div class="group flex w-56 items-center gap-2 rounded-xl border border-divider bg-surface-dim px-3 py-2 text-sm">
                              <Icon name={fileIcon(a)} size={20} class="shrink-0 text-muted" />
                              <div class="min-w-0 flex-1">
                                <div class="selectable truncate text-text" title={a.filename} dir="auto">{a.filename}</div>
                                <div class="text-xs text-faint">{formatSize(a.size)}</div>
                              </div>
                              <button
                                class="no-print grid size-8 place-items-center rounded-full text-muted hover:bg-hover"
                                title="Open"
                                aria-label={`Open ${a.filename}`}
                                onClick={() => p.onAttachment(m, a, true)}
                              >
                                <Icon name="openExternal" size={18} />
                              </button>
                              <button
                                class="no-print grid size-8 place-items-center rounded-full text-muted hover:bg-hover"
                                title="Save to Downloads"
                                aria-label={`Save ${a.filename}`}
                                onClick={() => p.onAttachment(m, a, false)}
                              >
                                <Icon name="download" size={18} />
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
          </For>
        </div>
      </div>
    </Show>
  );
}
