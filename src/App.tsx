import { createEffect, createResource, createSignal, on, onCleanup, onMount, Show } from "solid-js";
import TopBar from "./components/TopBar";
import Sidebar from "./components/Sidebar";
import ThreadList from "./components/ThreadList";
import Reader from "./components/Reader";
import SetupCard from "./components/SetupCard";
import ShortcutsHelp from "./components/ShortcutsHelp";
import Snackbar from "./components/Snackbar";
import { api, onEvent } from "./lib/api";
import { labelName } from "./lib/format";
import { installShortcuts } from "./lib/shortcuts";
import type { OpenTarget, SyncStatus, ThreadRow } from "./lib/types";

export default function App() {
  const [status, { refetch: refetchStatus }] = createResource(api.status);
  const [sync, setSync] = createSignal<SyncStatus | null>(null);
  const [label, setLabel] = createSignal("INBOX");
  const [query, setQuery] = createSignal("");
  const [debounced, setDebounced] = createSignal("");
  const [selected, setSelected] = createSignal<string | null>(null);
  const [cursor, setCursor] = createSignal(0);
  const [collapsed, setCollapsed] = createSignal(false);
  const [help, setHelp] = createSignal(false);
  const [snack, setSnack] = createSignal<string | null>(null);
  const [signInError, setSignInError] = createSignal<string | null>(null);
  const [tick, setTick] = createSignal(0); // bump to reload after "mail-updated"
  let searchInput: HTMLInputElement | undefined;

  const signedIn = () => !!status()?.account;

  const [labels, { refetch: refetchLabels }] = createResource(() => (signedIn() ? tick() + 1 : false), () => api.labels(), {
    initialValue: [],
  });
  const [threads] = createResource(
    () => (signedIn() ? { label: label(), q: debounced(), t: tick() } : false),
    ({ label, q }) => (q.trim() ? api.search(q) : api.threads(label)),
    { initialValue: [] },
  );
  const [messages] = createResource(
    () => (selected() ? { id: selected()!, t: tick() } : false),
    ({ id }) => api.thread(id),
    { initialValue: [] },
  );

  // Debounce search typing.
  createEffect(
    on(query, (q) => {
      const h = setTimeout(() => setDebounced(q), 150);
      onCleanup(() => clearTimeout(h));
    }),
  );
  createEffect(on([label, debounced], () => setCursor(0), { defer: true }));
  createEffect(() => {
    const s = status();
    if (s) setSync(s.sync);
  });

  let snackTimer: number | undefined;
  const toast = (text: string) => {
    setSnack(text);
    clearTimeout(snackTimer);
    snackTimer = window.setTimeout(() => setSnack(null), 2500);
  };

  const copyCode = async (code: string) => {
    try {
      await api.copyText(code);
      toast(`Code ${code} copied`);
    } catch {
      toast("Couldn't copy the code");
    }
  };

  const openThread = (t: ThreadRow, index: number) => {
    setCursor(index);
    setSelected(t.threadId);
  };

  const openTarget = (t: OpenTarget | null) => {
    if (!t) return;
    setQuery("");
    setLabel("INBOX");
    setSelected(t.threadId);
  };

  onMount(() => {
    // Register cleanups synchronously (inside the owner), then do async work.
    let disposed = false;
    let unlisteners: (() => void)[] = [];
    void Promise.all([
      onEvent<null>("mail-updated", () => setTick((n) => n + 1)),
      onEvent<SyncStatus>("sync-status", (s) => setSync(s)),
      onEvent<OpenTarget>("open-thread", (t) => {
        void api.takePendingOpen();
        openTarget(t);
      }),
    ]).then((u) => {
      if (disposed) u.forEach((f) => f());
      else unlisteners = u;
    });
    onCleanup(() => {
      disposed = true;
      unlisteners.forEach((u) => u());
    });
    void api.takePendingOpen().catch(() => null).then(openTarget);

    const remove = installShortcuts({
      next: () => setCursor((c) => Math.min(c + 1, Math.max(0, threads().length - 1))),
      prev: () => setCursor((c) => Math.max(c - 1, 0)),
      open: () => {
        const t = threads()[cursor()];
        if (t) openThread(t, cursor());
      },
      back: () => setSelected(null),
      focusSearch: () => searchInput?.focus(),
      goto: (id) => {
        setQuery("");
        setSelected(null);
        setLabel(id);
      },
      copyCode: () => {
        const code = [...messages()].reverse().find((m) => m.otpCode)?.otpCode ?? threads()[cursor()]?.otpCode;
        if (code) void copyCode(code);
      },
      refresh: () => void api.syncNow(),
      toggleHelp: () => setHelp((h) => !h),
    });
    onCleanup(remove);
  });

  const signIn = async () => {
    setSignInError(null);
    try {
      await api.signIn();
      await refetchStatus();
      setTick((n) => n + 1);
    } catch (e) {
      setSignInError((e as Error).message);
    }
  };

  const signOut = async () => {
    if (!confirm(`Sign out of ${status()?.account}? Cached mail on this PC will be removed.`)) return;
    await api.signOut().catch(() => {});
    setSelected(null);
    await refetchStatus();
  };

  const emptyText = () =>
    debounced().trim()
      ? `No messages match "${debounced().trim()}".`
      : sync()?.syncing
        ? "Fetching your mail…"
        : `No conversations in ${labelName(label(), labels().find((l) => l.id === label())?.name ?? label())}.`;

  return (
    <div class="flex h-full flex-col">
      <TopBar
        query={query()}
        onQuery={setQuery}
        onToggleSidebar={() => setCollapsed((c) => !c)}
        onRefresh={() => void api.syncNow()}
        onHelp={() => setHelp(true)}
        onSignOut={signOut}
        account={status()?.account ?? null}
        sync={sync()}
        searchRef={(el) => (searchInput = el)}
      />
      <div class="flex min-h-0 flex-1">
        <Sidebar
          labels={labels()}
          current={label()}
          collapsed={collapsed()}
          onSelect={(id) => {
            setQuery("");
            setSelected(null);
            setLabel(id);
            void refetchLabels();
          }}
        />
        <main class="mb-4 mr-4 flex min-w-0 flex-1 overflow-hidden rounded-2xl bg-surface">
          <Show
            when={status() && signedIn()}
            fallback={
              <Show when={status()}>
                <SetupCard
                  clientConfigured={status()!.clientConfigured}
                  clientConfigPath={status()!.clientConfigPath}
                  error={signInError()}
                  onSignIn={signIn}
                  onRecheck={() => void refetchStatus()}
                />
              </Show>
            }
          >
            <section class={`min-w-0 border-r border-divider ${selected() ? "hidden w-[44%] max-w-[640px] lg:block" : "flex-1"}`}>
              <ThreadList
                threads={threads()}
                selectedId={selected()}
                cursor={cursor()}
                loading={threads.loading}
                emptyText={emptyText()}
                onOpen={openThread}
                onCopyCode={copyCode}
              />
            </section>
            <Show when={selected()}>
              <section class="min-w-0 flex-1">
                <Reader messages={messages()} onBack={() => setSelected(null)} onCopyCode={copyCode} />
              </section>
            </Show>
          </Show>
        </main>
      </div>
      <Show when={help()}>
        <ShortcutsHelp onClose={() => setHelp(false)} />
      </Show>
      <Snackbar text={snack()} />
    </div>
  );
}
