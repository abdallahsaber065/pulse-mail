// Popover from the avatar: accounts (switch, unified inbox, add), settings,
// shortcuts, sign out.
import { createSignal, For, Show } from "solid-js";
import Avatar from "./Avatar";
import Icon, { type IconName } from "./Icon";
import Popover from "./ui/Popover";
import type { Account } from "../lib/types";

export default function AccountMenu(p: {
  accounts: Account[];
  /** Account in view; null = all inboxes. */
  current: number | null;
  live: boolean;
  onSelect(accountId: number | null): void;
  onAddAccount(): void;
  onSettings(): void;
  onShortcuts(): void;
  onSignOut(accountId: number): void;
}) {
  const [open, setOpen] = createSignal(false);
  const run = (f: () => void) => () => {
    setOpen(false);
    f();
  };
  const shown = () => p.accounts.find((a) => a.id === p.current) ?? p.accounts[0];
  const multi = () => p.accounts.length > 1;
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
  const Unread = (u: { n: number }) => (
    <Show when={u.n > 0}>
      <span class="ml-auto text-xs font-medium tabular-nums text-text">{u.n > 999 ? "999+" : u.n}</span>
    </Show>
  );

  return (
    <div class="relative ml-1">
      <button
        class={`rounded-full p-1 transition-shadow hover:bg-hover ${open() ? "ring-2 ring-primary" : ""}`}
        title={p.current == null && multi() ? `All inboxes\n${p.accounts.map((a) => a.email).join("\n")}` : `Google Account\n${shown()?.email ?? ""}`}
        onClick={() => setOpen((o) => !o)}
        aria-haspopup="menu"
        aria-expanded={open()}
      >
        <Show when={p.current == null && multi()} fallback={<Avatar name={shown()?.displayName || shown()?.email || "?"} email={shown()?.email ?? ""} color={shown()?.color} picture={shown()?.avatarData} size={32} />}>
          <span class="grid size-8 place-items-center rounded-full bg-secondary-container text-on-secondary-container">
            <Icon name="allInbox" size={18} />
          </span>
        </Show>
      </button>
      <Show when={open()}>
        <Popover label="Accounts" onClose={() => setOpen(false)} class="w-[340px] p-2">
          <Show when={shown()}>
            {(a) => (
              <div class="flex flex-col items-center gap-2 px-4 pb-4 pt-5 text-center">
                <div class="relative">
                  <Avatar name={a().displayName || a().email} email={a().email} color={a().color} picture={a().avatarData} size={64} />
                  <span
                    class={`absolute bottom-0.5 right-0.5 size-4 rounded-full border-2 border-surface-high ${p.live ? "bg-otp" : "bg-outline"}`}
                    title={p.live ? "Live" : "Not live"}
                  />
                </div>
                <div class="mt-1 max-w-full truncate text-base font-medium" dir="auto">Hi, {a().displayName || a().email.split("@")[0]}!</div>
                <div class="rounded-full bg-surface-container px-3 py-1 text-xs text-muted">
                  {multi() ? `${p.accounts.length} accounts · all notify` : "Signed in with Google"}
                </div>
              </div>
            )}
          </Show>
          <div class="border-t border-divider py-2" role="menu" aria-label="Switch account">
            <Show when={multi()}>
              <button
                class={`flex h-12 w-full items-center gap-3 rounded-full px-3 text-left text-sm hover:bg-hover ${p.current == null ? "bg-secondary-container/60" : ""}`}
                onClick={run(() => p.onSelect(null))}
                role="menuitemradio"
                aria-checked={p.current == null}
              >
                <span class="grid size-8 place-items-center rounded-full bg-surface-container text-muted">
                  <Icon name="allInbox" size={18} />
                </span>
                <span class="flex-1">All inboxes</span>
                <Unread n={p.accounts.reduce((n, a) => n + a.unread, 0)} />
              </button>
              <For each={p.accounts}>
                {(a) => (
                  <button
                    class={`flex h-12 w-full items-center gap-3 rounded-full px-3 text-left text-sm hover:bg-hover ${p.current === a.id ? "bg-secondary-container/60" : ""}`}
                    onClick={run(() => p.onSelect(a.id))}
                    role="menuitemradio"
                    aria-checked={p.current === a.id}
                  >
                    <Avatar name={a.displayName || a.email} email={a.email} color={a.color} picture={a.avatarData} size={32} />
                    <span class="min-w-0 flex-1">
                      <span class="block truncate">{a.email}</span>
                      <Show when={a.sync.error || (a.backfillState !== "done" && a.backfillState !== "setup")}>
                        <span class={`block truncate text-xs ${a.sync.error ? "text-danger" : "text-muted"}`}>
                          {a.sync.error ?? (a.backfillState === "paused" ? "Download paused" : "Downloading mail…")}
                        </span>
                      </Show>
                    </span>
                    <Unread n={a.unread} />
                  </button>
                )}
              </For>
            </Show>
            <Item icon="add" label="Add another account" onClick={run(p.onAddAccount)} />
          </div>
          <div class="border-t border-divider pt-2" role="menu">
            <Item icon="settings" label="Settings" hint="," onClick={run(p.onSettings)} />
            <Item icon="keyboard" label="Keyboard shortcuts" hint="?" onClick={run(p.onShortcuts)} />
            <Show when={!multi() && shown()}>
              <Item icon="logout" label="Sign out" onClick={run(() => p.onSignOut(shown()!.id))} danger />
            </Show>
          </div>
        </Popover>
      </Show>
    </div>
  );
}
