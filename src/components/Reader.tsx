import { createSignal, For, Show } from "solid-js";
import Icon from "./Icon";
import Avatar from "./Avatar";
import EmailFrame from "./EmailFrame";
import { fullDate, labelName } from "../lib/format";
import { theme } from "../lib/theme";
import type { Message } from "../lib/types";

interface Props {
  messages: Message[];
  onBack(): void;
  onCopyCode(code: string): void;
}

const HIDDEN_LABELS = new Set(["UNREAD", "STARRED", "CATEGORY_PERSONAL", "CATEGORY_UPDATES", "CATEGORY_PROMOTIONS", "CATEGORY_SOCIAL", "CATEGORY_FORUMS", "IMPORTANT", "SENT"]);

export default function Reader(p: Props) {
  const [imagesFor, setImagesFor] = createSignal<Set<string>>(new Set());
  const first = () => p.messages[0];
  const otp = () => [...p.messages].reverse().find((m) => m.otpCode)?.otpCode ?? null;
  const hasHtmlImages = (m: Message) => !!m.bodyHtml && /<img\b[^>]*\bsrc\s*=\s*["']?https?:/i.test(m.bodyHtml);

  return (
    <Show when={first()} fallback={<div class="grid h-full place-items-center text-sm text-muted">Select a conversation to read it.</div>}>
      <div class="h-full overflow-y-auto">
        <div class="sticky top-0 z-10 flex h-12 items-center gap-1 border-b border-divider bg-surface px-2">
          <button class="grid size-10 place-items-center rounded-full text-muted hover:bg-hover" onClick={p.onBack} title="Back to list (u)">
            <Icon name="back" />
          </button>
        </div>

        <div class="px-6 pb-10 pt-4">
          <div class="mb-4 flex flex-wrap items-center gap-2 pl-14">
            <h1 class="selectable font-display text-[22px] leading-tight text-text">{first().subject || "(no subject)"}</h1>
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
                class="flex items-center gap-2 rounded-full bg-otp px-4 py-2 text-sm font-medium text-surface hover:brightness-110"
                onClick={() => p.onCopyCode(otp()!)}
                title="Copy code (c)"
              >
                <Icon name="copy" size={18} /> Copy
              </button>
            </div>
          </Show>

          <For each={p.messages}>
            {(m) => (
              <article class="mb-6">
                <div class="mb-3 flex items-start gap-3">
                  <Avatar name={m.fromName} email={m.fromEmail} />
                  <div class="min-w-0 flex-1">
                    <div class="flex items-baseline gap-2">
                      <span class="selectable truncate font-semibold text-text">{m.fromName || m.fromEmail}</span>
                      <span class="selectable truncate text-xs text-muted">&lt;{m.fromEmail}&gt;</span>
                      <span class="ml-auto shrink-0 text-xs text-muted">{fullDate(m.internalDate)}</span>
                    </div>
                    <div class="truncate text-xs text-muted">to {m.toList || "me"}</div>
                  </div>
                </div>
                <Show when={hasHtmlImages(m) && !imagesFor().has(m.id)}>
                  <div class="mb-2 ml-14 flex items-center gap-3 rounded-lg bg-surface-container px-3 py-2 text-xs text-muted">
                    <Icon name="image" size={16} />
                    <span class="flex-1">Images are hidden to protect your privacy.</span>
                    <button
                      class="font-medium text-primary hover:underline"
                      onClick={() => setImagesFor((s) => new Set(s).add(m.id))}
                    >
                      Show images
                    </button>
                  </div>
                </Show>
                <div class="pl-14">
                  <EmailFrame html={m.bodyHtml} text={m.bodyText} dark={theme.isDark()} allowRemoteImages={imagesFor().has(m.id)} />
                </div>
              </article>
            )}
          </For>
        </div>
      </div>
    </Show>
  );
}
