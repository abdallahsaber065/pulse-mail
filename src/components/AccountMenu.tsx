// Popover from the avatar: who is signed in, settings, shortcuts, sign out.
import { createSignal, Show } from "solid-js";
import Avatar from "./Avatar";
import Icon, { type IconName } from "./Icon";
import Popover from "./ui/Popover";

export default function AccountMenu(p: {
  account: string;
  live: boolean;
  onSettings(): void;
  onShortcuts(): void;
  onSignOut(): void;
}) {
  const [open, setOpen] = createSignal(false);
  const run = (f: () => void) => () => {
    setOpen(false);
    f();
  };
  const Item = (i: { icon: IconName; label: string; hint?: string; onClick(): void; danger?: boolean }) => (
    <button
      class={`flex h-11 w-full items-center gap-4 rounded-full px-4 text-left text-sm hover:bg-hover ${i.danger ? "text-danger" : "text-text"}`}
      onClick={i.onClick}
      role="menuitem"
    >
      <Icon name={i.icon} size={20} />
      <span class="flex-1">{i.label}</span>
      <Show when={i.hint}>
        <kbd class="font-mono text-xs text-faint">{i.hint}</kbd>
      </Show>
    </button>
  );

  return (
    <div class="relative ml-1">
      <button
        class={`rounded-full p-1 transition-shadow hover:bg-hover ${open() ? "ring-2 ring-primary" : ""}`}
        title={`Google Account\n${p.account}`}
        onClick={() => setOpen((o) => !o)}
        aria-haspopup="menu"
        aria-expanded={open()}
      >
        <Avatar name={p.account} email={p.account} size={32} />
      </button>
      <Show when={open()}>
        <Popover label="Account" onClose={() => setOpen(false)} class="w-[320px] p-2">
          <div class="flex flex-col items-center gap-2 px-4 pb-4 pt-5 text-center">
            <div class="relative">
              <Avatar name={p.account} email={p.account} size={72} />
              <span
                class={`absolute bottom-0.5 right-0.5 size-4 rounded-full border-2 border-surface-high ${p.live ? "bg-otp" : "bg-outline"}`}
                title={p.live ? "Live" : "Not live"}
              />
            </div>
            <div class="mt-1 text-base font-medium">Hi, {p.account.split("@")[0]}!</div>
            <div class="selectable max-w-full truncate text-sm text-muted">{p.account}</div>
            <div class="rounded-full bg-surface-container px-3 py-1 text-xs text-muted">
              Signed in with Google · token in Windows Credential Manager
            </div>
          </div>
          <div class="border-t border-divider pt-2" role="menu">
            <Item icon="settings" label="Settings" hint="," onClick={run(p.onSettings)} />
            <Item icon="keyboard" label="Keyboard shortcuts" hint="?" onClick={run(p.onShortcuts)} />
            <Item icon="logout" label="Sign out" onClick={run(p.onSignOut)} danger />
          </div>
        </Popover>
      </Show>
    </div>
  );
}
