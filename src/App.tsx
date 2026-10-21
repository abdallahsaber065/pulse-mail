import { batch, createEffect, createMemo, createResource, createSignal, on, onCleanup, onMount, Show } from "solid-js";
import TopBar from "./components/TopBar";
import Sidebar from "./components/Sidebar";
import ThreadList, { type RowActions } from "./components/ThreadList";
import Reader from "./components/Reader";
import SetupCard from "./components/SetupCard";
import ShortcutsHelp from "./components/ShortcutsHelp";
import SettingsPanel from "./components/SettingsPanel";
import { TopProgress } from "./components/SyncIndicator";
import { snack, SnackbarHost } from "./components/Snackbar";
import { confirmDialog, DialogHost } from "./components/ui/dialog";
import type { IconName } from "./components/Icon";
import { api, onEvent } from "./lib/api";
import { labelName } from "./lib/format";
import { checkLink } from "./lib/links";
import { installShortcuts } from "./lib/shortcuts";
import type { Activity, Attachment, Message, NotificationMode, OpenTarget, PrefKey, Prefs, SyncStatus, ThreadRow } from "./lib/types";

type ThreadRef = Pick<ThreadRow, "threadId" | "unread" | "starred">;

/** Optimistic per-thread state shown until the next reload confirms it. */
interface Override {
  unread?: boolean;
  starred?: boolean;
  removed?: boolean;
}

export default function App() {
  const [status, { refetch: refetchStatus }] = createResource(api.status);
  const [sync, setSync] = createSignal<SyncStatus | null>(null);
  const [activity, setActivity] = createSignal<Activity[]>([]);
  const [online, setOnline] = createSignal(navigator.onLine);
  const [label, setLabel] = createSignal("INBOX");
  const [query, setQuery] = createSignal("");
  const [debounced, setDebounced] = createSignal("");
  const [selected, setSelected] = createSignal<string | null>(null);
  const [cursor, setCursor] = createSignal(0);
  const [collapsed, setCollapsed] = createSignal(false);
  const [help, setHelp] = createSignal(false);
  const [settings, setSettings] = createSignal(false);
  const [signInError, setSignInError] = createSignal<string | null>(null);
  const [tick, setTick] = createSignal(0); // bump to reload after "mail-updated"
  const [overrides, setOverrides] = createSignal<Record<string, Override>>({});
  const [msgs, setMsgs] = createSignal<Message[]>([]);
  const [msgLoading, setMsgLoading] = createSignal(false);
  /** Body download for the open thread failed (offline, Gmail error). */
  const [bodyError, setBodyError] = createSignal<string | null>(null);
  let searchInput: HTMLInputElement | undefined;

  const signedIn = () => !!status()?.account;
  const inInbox = () => label() === "INBOX" && !debounced().trim();

  const [labels, { refetch: refetchLabels }] = createResource(() => (signedIn() ? tick() + 1 : false), () => api.labels(), {
    initialValue: [],
  });
  // Infinite scroll: the list grows a page at a time as you near the end.
  const PAGE = 200;
  const [limit, setLimit] = createSignal(PAGE);
  const [threads] = createResource(
    () => (signedIn() ? { label: label(), q: debounced(), t: tick(), n: limit() } : false),
    ({ label, q, n }) => (q.trim() ? api.search(q) : api.threads(label, n)),
    { initialValue: [] },
  );
  const loadMore = () => {
    if (!debounced().trim() && !threads.loading && threads().length >= limit()) setLimit((n) => n + PAGE);
  };

  const rows = createMemo(() => {
    const o = overrides();
    return threads()
      .filter((t) => !o[t.threadId]?.removed)
      .map((t) => {
        const x = o[t.threadId];
        return x ? { ...t, unread: x.unread ?? t.unread, starred: x.starred ?? t.starred } : t;
      });
  });
  const messages = createMemo(() => {
    const x = selected() ? overrides()[selected()!] : undefined;
    return x ? msgs().map((m) => ({ ...m, unread: x.unread ?? m.unread, starred: x.starred ?? m.starred })) : msgs();
  });

  // Overrides whose server call finished are dropped on the next reload.
  const settled = new Set<string>();
  createEffect(
    on(threads, () => {
      if (settled.size === 0) return;
      setOverrides((o) => {
        const n = { ...o };
        settled.forEach((id) => delete n[id]);
        return n;
      });
      settled.clear();
    }),
  );

  // Debounce search typing.
  createEffect(
    on(query, (q) => {
      const h = setTimeout(() => setDebounced(q), 150);
      onCleanup(() => clearTimeout(h));
    }),
  );
  createEffect(
    on(
      [label, debounced],
      () => {
        setCursor(0);
        setLimit(PAGE);
      },
      { defer: true },
    ),
  );
  createEffect(() => {
    const s = status();
    if (s) setSync(s.sync);
  });
  createEffect(
    on(signedIn, (yes) => {
      if (yes) void api.activity().then(setActivity, () => {});
    }),
  );

  // Reader: show what is cached immediately, then fetch missing bodies.
  let loadSeq = 0;
  const loadThread = async (id: string, fetchBodies: boolean) => {
    const seq = ++loadSeq;
    const cached = await api.thread(id).catch(() => [] as Message[]);
    if (seq !== loadSeq) return;
    if (cached.length) setMsgs(cached);
    if (fetchBodies && (cached.length === 0 || cached.some((m) => !m.bodyLoaded))) {
      setBodyError(null);
      try {
        const full = await api.loadThread(id);
        if (seq === loadSeq) setMsgs(full);
      } catch (e) {
        if (seq === loadSeq) setBodyError((e as Error).message);
      }
    }
    if (seq === loadSeq) setMsgLoading(false);
  };
  createEffect(
    on(selected, (id) => {
      setMsgs([]);
      setBodyError(null);
      if (id) {
        setMsgLoading(true);
        void loadThread(id, true);
      }
    }),
  );
  createEffect(
    on(
      tick,
      () => {
        const id = selected();
        if (id) void loadThread(id, false);
      },
      { defer: true },
    ),
  );

  // Backend prefs, mirrored for instant (optimistic) updates.
  const [prefs, setPrefs] = createSignal<Prefs | null>(null);
  createEffect(() => {
    const s = status();
    if (s) setPrefs(s.prefs);
  });
  const setPref = async <K extends PrefKey>(key: K, value: Prefs[K]) => {
    const prev = prefs();
    if (prev) setPrefs({ ...prev, [key]: value });
    try {
      await api.setPref(key, value);
    } catch (e) {
      setPrefs(prev);
      snack((e as Error).message);
    }
  };

  /** Link clicked inside an email: phishing check, then the system browser. */
  const openLink = async (href: string, text: string) => {
    if (/^mailto:/i.test(href)) {
      void api.openExternal(href).catch((e: Error) => snack(e.message));
      return;
    }
    const c = checkLink(href, text);
    if (!c) {
      snack("That link can't be opened.");
      return;
    }
    if (c.mismatch || c.punycode) {
      const ok = await confirmDialog({
        title: "Check this link",
        icon: "shield",
        body: (
          <div class="space-y-2">
            <Show when={c.mismatch}>
              <p>
                The link says <b class="selectable">{c.shown}</b> but opens <b class="selectable">{c.host}</b>.
              </p>
            </Show>
            <Show when={c.punycode}>
              <p>
                The address <b class="selectable">{c.host}</b> uses look-alike characters.
              </p>
            </Show>
            <p class="text-xs">Phishing emails often disguise links like this. Only continue if you trust the sender.</p>
          </div>
        ),
        confirmLabel: "Open anyway",
        danger: true,
      });
      if (!ok) return;
    }
    void api.openExternal(href).catch((e: Error) => snack(e.message));
  };

  const attachment = async (m: Message, a: Attachment, open: boolean) => {
    snack(open ? `Opening ${a.filename}…` : `Saving ${a.filename}…`);
    try {
      const r = await api.saveAttachment(m.id, a.part, open);
      if (r.blocked) snack(`${a.filename} can run programs, so it was saved but not opened.`, { duration: 7000 });
      else if (!r.opened) snack(`Saved to Downloads`);
    } catch (e) {
      snack((e as Error).message);
    }
  };

  const copyCode = async (code: string) => {
    try {
      await api.copyText(code, true);
      snack(`Code ${code} copied to clipboard`);
    } catch {
      snack("Couldn't copy the code");
    }
  };

  // ---- Thread actions (optimistic, with rollback) ----
  const modify = async (threadId: string, change: Override, add: string[], remove: string[]) => {
    const prev = overrides()[threadId];
    setOverrides((o) => ({ ...o, [threadId]: { ...o[threadId], ...change } }));
    try {
      await api.modifyThread(threadId, add, remove);
      settled.add(threadId);
      setTick((n) => n + 1);
      return true;
    } catch (e) {
      setOverrides((o) => {
        const n = { ...o };
        if (prev) n[threadId] = prev;
        else delete n[threadId];
        return n;
      });
      snack((e as Error).message);
      return false;
    }
  };

  const archive = (t: ThreadRef) => {
    if (selected() === t.threadId) setSelected(null);
    void modify(t.threadId, { removed: true }, [], ["INBOX"]).then((ok) => {
      if (ok)
        snack("Conversation archived", {
          action: { label: "Undo", run: () => void modify(t.threadId, { removed: false }, ["INBOX"], []) },
        });
    });
  };
  const setRead = (t: ThreadRef, read: boolean) => {
    if (t.unread === !read) return;
    if (!read && selected() === t.threadId) setSelected(null); // Gmail closes on "mark unread"
    void modify(t.threadId, { unread: !read }, read ? [] : ["UNREAD"], read ? ["UNREAD"] : []);
  };
  const toggleStar = (t: ThreadRef) =>
    void modify(t.threadId, { starred: !t.starred }, t.starred ? [] : ["STARRED"], t.starred ? ["STARRED"] : []);

  const actions: RowActions = {
    get archive() {
      return inInbox() ? archive : undefined;
    },
    toggleRead: (t) => setRead(t, t.unread),
    toggleStar,
  };
  /** The open thread, else the row under the keyboard cursor. */
  const current = (): ThreadRef | undefined => {
    const id = selected();
    if (!id) return rows()[cursor()];
    const row = rows().find((r) => r.threadId === id);
    if (row) return row;
    const m = messages();
    return m.length ? { threadId: id, unread: m.some((x) => x.unread), starred: m.some((x) => x.starred) } : undefined;
  };

  const openThread = (t: ThreadRow, index: number) => {
    setCursor(index);
    setSelected(t.threadId);
    if (t.unread) setRead(t, true);
  };

  const openTarget = (t: OpenTarget | null) => {
    if (!t) return;
    batch(() => {
      setQuery("");
      setLabel("INBOX");
      setSelected(t.threadId);
    });
  };

  const goto = (id: string) =>
    batch(() => {
      setQuery("");
      setSelected(null);
      setLabel(id);
    });

  onMount(() => {
    // Register cleanups synchronously (inside the owner), then do async work.
    let disposed = false;
    let unlisteners: (() => void)[] = [];
    void Promise.all([
      onEvent<null>("mail-updated", () => setTick((n) => n + 1)),
      onEvent<SyncStatus>("sync-status", (s) => setSync(s)),
      onEvent<Activity>("sync-activity", (a) => setActivity((xs) => [...xs.slice(-59), a])),
      onEvent<OpenTarget>("open-thread", (t) => {
        void api.takePendingOpen();
        openTarget(t);
      }),
    ]).then((u) => {
      if (disposed) u.forEach((f) => f());
      else unlisteners = u;
    });
    const goOnline = () => {
      setOnline(true);
      void api.syncNow(false);
    };
    const goOffline = () => setOnline(false);
    window.addEventListener("online", goOnline);
    window.addEventListener("offline", goOffline);
    onCleanup(() => {
      disposed = true;
      unlisteners.forEach((u) => u());
      window.removeEventListener("online", goOnline);
      window.removeEventListener("offline", goOffline);
    });
    void api.takePendingOpen().catch(() => null).then(openTarget);

    const withCurrent = (f: (t: ThreadRef) => void) => () => {
      const t = current();
      if (t) f(t);
    };
    const remove = installShortcuts({
      next: () => setCursor((c) => Math.min(c + 1, Math.max(0, rows().length - 1))),
      prev: () => setCursor((c) => Math.max(c - 1, 0)),
      open: () => {
        const t = rows()[cursor()];
        if (t) openThread(t, cursor());
      },
      back: () => setSelected(null),
      focusSearch: () => searchInput?.focus(),
      goto,
      copyCode: () => {
        const code = [...messages()].reverse().find((m) => m.otpCode)?.otpCode ?? rows()[cursor()]?.otpCode;
        if (code) void copyCode(code);
      },
      refresh: () => void api.syncNow(),
      toggleHelp: () => setHelp((h) => !h),
      archive: withCurrent((t) => (inInbox() ? archive(t) : snack("Archive works from the Inbox"))),
      star: withCurrent(toggleStar),
      markRead: withCurrent((t) => setRead(t, true)),
      markUnread: withCurrent((t) => setRead(t, false)),
      settings: () => setSettings(true),
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
    const account = status()?.account;
    const ok = await confirmDialog({
      icon: "logout",
      title: "Sign out of Pulse Mail?",
      body: (
        <>
          <span class="font-medium text-text">{account}</span> will be signed out on this PC and the mail cached here will be
          removed. Your Gmail account isn't affected, and you can sign back in anytime.
        </>
      ),
      confirmLabel: "Sign out",
      danger: true,
    });
    if (!ok) return;
    try {
      await api.signOut();
      snack("Signed out");
    } catch (e) {
      snack((e as Error).message);
    }
    setSelected(null);
    setActivity([]);
    await refetchStatus();
  };

  const updateSetting = async (f: () => Promise<unknown>) => {
    try {
      await f();
    } catch (e) {
      snack((e as Error).message);
    }
    await refetchStatus();
  };

  const empty = (): { icon: IconName; title: string; hint: string } => {
    const q = debounced().trim();
    if (q) return { icon: "searchOff", title: `No results for "${q}"`, hint: "Search covers the mail saved on this PC (last 90 days)." };
    if (label() === "INBOX") return { icon: "doneAll", title: "You're all caught up", hint: "New mail shows up here the moment it arrives." };
    const name = labelName(label(), labels().find((l) => l.id === label())?.name ?? label());
    return { icon: "inboxEmpty", title: `Nothing in ${name}`, hint: "Pulse keeps the last 90 days of mail on this PC." };
  };
  const listLoading = () =>
    rows().length === 0 && (threads.loading || (!debounced().trim() && (!!sync()?.syncing || !!sync()?.progress)));

  return (
    <div class="flex h-full flex-col">
      <TopProgress sync={signedIn() ? sync() : null} online={online()} />
      <TopBar
        query={query()}
        onQuery={setQuery}
        onToggleSidebar={() => setCollapsed((c) => !c)}
        onRefresh={() => void api.syncNow()}
        onHelp={() => setHelp(true)}
        onSettings={() => setSettings(true)}
        onSignOut={() => void signOut()}
        account={status()?.account ?? null}
        sync={sync()}
        online={online()}
        activity={activity()}
        searchRef={(el) => (searchInput = el)}
      />
      <div class="flex min-h-0 flex-1">
        <Sidebar
          labels={labels()}
          current={label()}
          collapsed={collapsed()}
          onSelect={(id) => {
            goto(id);
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
            <section class={`thread-list-pane min-w-0 border-r border-divider ${selected() ? "hidden w-[44%] max-w-[640px] lg:block" : "flex-1"}`}>
              <ThreadList
                threads={rows()}
                selectedId={selected()}
                cursor={cursor()}
                loading={listLoading()}
                empty={empty()}
                actions={actions}
                onOpen={openThread}
                onEndReached={loadMore}
                onCopyCode={(c) => void copyCode(c)}
              />
            </section>
            <Show when={selected()}>
              <section class="reader-pane min-w-0 flex-1">
                <Reader
                  messages={messages()}
                  loading={msgLoading()}
                  error={bodyError()}
                  onRetry={() => {
                    const id = selected();
                    if (id) void loadThread(id, true);
                  }}
                  onBack={() => setSelected(null)}
                  onCopyCode={(c) => void copyCode(c)}
                  onArchive={
                    inInbox()
                      ? () => {
                          const t = current();
                          if (t) archive(t);
                        }
                      : undefined
                  }
                  onToggleRead={() => {
                    const t = current();
                    if (t) setRead(t, t.unread);
                  }}
                  onToggleStar={() => {
                    const t = current();
                    if (t) toggleStar(t);
                  }}
                  onLink={(href, text) => void openLink(href, text)}
                  onAttachment={(m, a, open) => void attachment(m, a, open)}
                  emailDark={prefs()?.email_dark ?? "smart"}
                  onToggleEmailDark={() => void setPref("email_dark", prefs()?.email_dark === "original" ? "smart" : "original")}
                />
              </section>
            </Show>
          </Show>
        </main>
      </div>
      <Show when={help()}>
        <ShortcutsHelp onClose={() => setHelp(false)} />
      </Show>
      <Show when={settings() && status()}>
        <SettingsPanel
          status={status()!}
          onClose={() => setSettings(false)}
          onAutostart={(v) => void updateSetting(() => api.setAutostart(v))}
          onCloseBehavior={(quit) => void updateSetting(() => api.setDestroyOnClose(quit))}
          onNotifications={(m: NotificationMode) => void updateSetting(() => api.setNotifications(m))}
          prefs={prefs() ?? status()!.prefs}
          onPref={(k, v) => void setPref(k, v)}
        />
      </Show>
      <DialogHost />
      <SnackbarHost />
    </div>
  );
}
