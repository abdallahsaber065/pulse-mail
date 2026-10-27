// Stand-in for the native OTP toast. Only mounted in demo mode (browser
// mock or `PULSE_MAIL_DEMO=1` in a dev build). Release builds never import
// this through a live path.
import { createSignal } from "solid-js";
import Icon from "./Icon";

export default function DemoToast(p: {
  from: string;
  code: string;
  subject: string;
  onCopy(): void;
  onOpen(): void;
  onDismiss(): void;
}) {
  const [copied, setCopied] = createSignal(false);

  return (
    <aside
      class="animate-fade-in fixed right-5 bottom-5 z-[80] w-[360px] overflow-hidden rounded-xl border border-divider bg-surface-high shadow-2xl"
      role="status"
      aria-label="Demo notification"
      data-demo-toast
    >
      <div class="flex items-center gap-2 border-b border-divider px-3 py-2">
        <img src="/icon.png" alt="" class="size-5 rounded" />
        <span class="flex-1 text-xs font-medium text-muted">Pulse Mail</span>
        <button
          class="grid size-7 place-items-center rounded-full text-faint hover:bg-hover hover:text-text"
          onClick={p.onDismiss}
          title="Dismiss"
          aria-label="Dismiss notification"
        >
          <Icon name="close" size={16} />
        </button>
      </div>
      <div class="px-4 py-3">
        <div class="font-mono text-[15px] font-semibold tracking-wide text-text">
          {p.code}
          <span class="font-sans font-medium tracking-normal text-muted"> · {p.from}</span>
        </div>
        <div class="mt-0.5 truncate text-sm text-text">{p.subject}</div>
        <div class="mt-1 text-xs text-faint">Verification code. Use Copy code to copy it.</div>
      </div>
      <div class="flex gap-2 px-3 pb-3">
        <button
          class="flex flex-1 items-center justify-center gap-1.5 rounded-full bg-otp px-3 py-1.5 text-sm font-medium text-surface hover:brightness-110"
          data-demo-copy
          onClick={() => {
            p.onCopy();
            setCopied(true);
          }}
        >
          <Icon name={copied() ? "checkCircle" : "copy"} size={16} />
          {copied() ? "Copied" : "Copy code"}
        </button>
        <button
          class="flex flex-1 items-center justify-center gap-1.5 rounded-full bg-surface-container px-3 py-1.5 text-sm font-medium text-text hover:bg-hover"
          data-demo-open
          onClick={p.onOpen}
        >
          Open
        </button>
      </div>
      <p class="px-4 pb-2 text-[10px] text-faint">Demo mailbox — not a real notification</p>
    </aside>
  );
}
