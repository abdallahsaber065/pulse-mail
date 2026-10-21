import { batch, createEffect, createMemo, createResource, createSignal, on, onCleanup, onMount, Show } from "solid-js";
import TopBar from "./components/TopBar";
import Sidebar from "./components/Sidebar";
import ThreadList, { type RowActions } from "./components/ThreadList";
import Reader from "./components/Reader";
import Onboarding from "./components/Onboarding";
import ShortcutsHelp from "./components/ShortcutsHelp";
import SettingsPanel from "./components/SettingsPanel";
import { TopProgress } from "./components/SyncIndicator";
import { snack, SnackbarHost } from "./components/Snackbar";
import { confirmDialog, DialogHost } from "./components/ui/dialog";
import type { IconName } from "./components/Icon";
import { api, onEvent } from "./lib/api";
import { labelName } from "./lib/format";
import { checkLink } from "./lib/links";
import { combineSync, windowLabel } from "./lib/accounts";
import { installShortcuts } from "./lib/shortcuts";
import type {
  Account,
  Activity,
  Attachment,
  Message,
  NotificationMode,
  OpenTarget,
  PrefKey,
  Prefs,
  SyncStatus,
  SyncWindow,
  ThreadRow,
} from "./lib/types";

type ThreadRef = Pick<ThreadRow, "threadId" | "unread" | "starred"> & { accountId: number | null };

const VIEW_KEY = "pm.accountView";

/** Optimistic per-thread state shown until the next reload confirms it. */
interface Override {
  unread?: boolean;
  starred?: boolean;
  removed?: boolean;
}

export default function App() {
  const [status, { refetch: refetchStatus }] = createResource(api.status);
  /** Live sync status per account (from sync-status events). */
  const [syncs, setSyncs] = createSignal<Record<number, SyncStatus>>({});
  /** Account in view; null = all inboxes. */
  const [accountView, setAccountView] = createSignal<number | null>(
    (() => {
      const v = Number(localStorage.getItem(VIEW_KEY));
      return Number.isFinite(v) && v > 0 ? v : null;
    })(),
  );
  /** Account of the open thread. */
  const [selectedAccount, setSelectedAccount] = createSignal<number | null>(null);
  const [onboarding, setOnboarding] = createSignal<{ mode: "first" | "add"; resume?: { accountId: number; email: string } } | null>(null);
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
  const [tick, setTick] = createSignal(0); // bump to reload after "mail-updated"
  const [overrides, setOverrides] = createSignal<Record<string, Override>>({});
  const [msgs, setMsgs] = createSignal<Message[]>([]);
  const [msgLoading, setMsgLoading] = createSignal(false);
  /** Body download for the open thread failed (offline, Gmail error). */
  const [bodyError, setBodyError] = createSignal<string | null>(null);
  let searchInput: HTMLInputElement | undefined;

  /** Accounts with their live status merged in. */
  const accounts = createMemo<Account[]>(() => {
    const live = syncs();
    return (status()?.accounts ?? []).map((a) => {
      const sync = live[a.id] ?? a.sync;
      const h = sync.history;
      return {
        ...a,
        sync,
        backfillState: h?.state ?? a.backfillState,
        syncWindow: h?.window ?? a.syncWindow,
      };
    });
  });
  const sync = createMemo(() => combineSync(accounts().map((a) => a.sync)));
  const signedIn = () => accounts().length > 0;
  // Forget a view of an account that was removed.
  createEffect(() => {
    const v = accountView();
    if (v != null && status() && !accounts().some((a) => a.id === v)) setAccountView(null);
  });
  createEffect(() => {
    const v = accountView();
    if (v == null) localStorage.removeItem(VIEW_KEY);
    else localStorage.setItem(VIEW_KEY, String(v));
  });
  const inInbox = () => label() === "INBOX" && !debounced().trim();

  const [labels, { refetch: refetchLabels }] = createResource(
    () => (signedIn() ? { t: tick(), a: accountView() } : false),
    ({ a }) => api.labels(a),
    { initialValue: [] },
  );
  // Infinite scroll: the list grows a page at a time as you near the end.
  const PAGE = 200;
  const [limit, setLimit] = createSignal(PAGE);
  const [threads] = createResource(
    () => (signedIn() ? { label: label(), q: debounced(), t: tick(), n: limit(), a: accountView() } : false),
    ({ label, q, n, a }) => (q.trim() ? api.search(q, 100, a) : api.threads(label, n, 0, a)),
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
      [label, debounced, accountView],
      () => {
        setCursor(0);
        setLimit(PAGE);
      },
      { defer: true },
    ),
  );
  createEffect(() => {
    const s = status();
    if (s) setSyncs(Object.fromEntries(s.accounts.map((a) => [a.id, a.sync])));
  });
  // First run: no account yet. Also resume an account whose setup was
  // interrupted (signed in, but the download range was never chosen).
  let resumeChecked = false;
  createEffect(() => {
    const s = status();
    if (!s || onboarding()) return;
    if (s.accounts.length === 0) {
      setOnboarding({ mode: "first" });
      return;
    }
    if (!resumeChecked) {
      resumeChecked = true;
      const pending = s.accounts.find((a) => a.backfillState === "setup");
      if (pending) setOnboarding({ mode: "add", resume: { accountId: pending.id, email: pending.email } });
    }
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
    const acct = selectedAccount();
    const cached = await api.thread(id, acct).catch(() => [] as Message[]);
    if (seq !== loadSeq) return;
    if (cached.length) setMsgs(cached);
    if (fetchBodies && (cached.length === 0 || cached.some((m) => !m.bodyLoaded))) {
      setBodyError(null);
      try {
        const full = await api.loadThread(id, acct);
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
      const r = await api.saveAttachment(m.id, a.part, open, selectedAccount());
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
  const modify = async (t: ThreadRef, change: Override, add: string[], remove: string[]) => {
    const threadId = t.threadId;
    const prev = overrides()[threadId];
    setOverrides((o) => ({ ...o, [threadId]: { ...o[threadId], ...change } }));
    try {
      await api.modifyThread(threadId, add, remove, t.accountId);
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
    void modify(t, { removed: true }, [], ["INBOX"]).then((ok) => {
      if (ok)
        snack("Conversation archived", {
          action: { label: "Undo", run: () => void modify(t, { removed: false }, ["INBOX"], []) },
        });
    });
  };
  const setRead = (t: ThreadRef, read: boolean) => {
    if (t.unread === !read) return;
    if (!read && selected() === t.threadId) setSelected(null); // Gmail closes on "mark unread"
    void modify(t, { unread: !read }, read ? [] : ["UNREAD"], read ? ["UNREAD"] : []);
  };
  const toggleStar = (t: ThreadRef) =>
    void modify(t, { starred: !t.starred }, t.starred ? [] : ["STARRED"], t.starred ? ["STARRED"] : []);

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
    const row = rows().find((r) => r.threadId === id && r.accountId === (selectedAccount() ?? r.accountId));
    if (row) return row;
    const m = messages();
    return m.length
      ? { threadId: id, accountId: selectedAccount(), unread: m.some((x) => x.unread), starred: m.some((x) => x.starred) }
      : undefined;
  };

  const openThread = (t: ThreadRow, index: number) => {
    setCursor(index);
    batch(() => {
      setSelectedAccount(t.accountId);
      setSelected(t.threadId);
    });
    if (t.unread) setRead(t, true);
  };

  const openTarget = (t: OpenTarget | null) => {
    if (!t) return;
    batch(() => {
      setQuery("");
      setLabel("INBOX");
      const v = accountView();
      if (v != null && v !== t.accountId) setAccountView(t.accountId);
      setSelectedAccount(t.accountId);
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
      onEvent<SyncStatus>("sync-status", (s) => {
        setSyncs((m) => ({ ...m, [s.accountId]: s }));
        // A status for an account we don't know yet (just added): reload.
        if (status() && !status()!.accounts.some((a) => a.id === s.accountId)) void refetchStatus();
      }),
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

  const removeAccount = async (accountId: number) => {
    const account = accounts().find((a) => a.id === accountId)?.email ?? "This account";
    const only = accounts().length === 1;
    const ok = await confirmDialog({
      icon: "logout",
      title: only ? "Sign out of Pulse Mail?" : "Remove this account?",
      body: (
        <>
          <span class="font-medium text-text">{account}</span> will be signed out on this PC and the mail cached here for it will be
          removed. Your Gmail account isn't affected, and you can add it back anytime.
        </>
      ),
      confirmLabel: only ? "Sign out" : "Remove",
      danger: true,
    });
    if (!ok) return;
    try {
      await api.signOut(accountId);
      snack(only ? "Signed out" : `Removed ${account}`);
    } catch (e) {
      snack((e as Error).message);
    }
    batch(() => {
      if (selectedAccount() === accountId) setSelected(null);
      if (accountView() === accountId) setAccountView(null);
      setSyncs((m) => {
        const n = { ...m };
        delete n[accountId];
        return n;
      });
    });
    if (only) setActivity([]);
    await refetchStatus();
    setTick((n) => n + 1);
  };

  const pauseHistory = async (accountId: number, pause: boolean) => {
    try {
      if (pause) await api.pauseHistory(accountId);
      else await api.resumeHistory(accountId);
    } catch (e) {
      snack((e as Error).message);
    }
  };

  const setSyncWindow = async (accountId: number, w: SyncWindow) => {
    try {
      await api.setSyncWindow(accountId, w);
      snack(`Downloading ${windowLabel(w).toLowerCase()} for this account`);
    } catch (e) {
      snack((e as Error).message);
    }
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
    if (q) return { icon: "searchOff", title: `No results for "${q}"`, hint: "Search covers the mail saved on this PC." };
    if (label() === "INBOX") return { icon: "doneAll", title: "You're all caught up", hint: "New mail shows up here the moment it arrives." };
    const name = labelName(label(), labels().find((l) => l.id === label())?.name ?? label());
    return { icon: "inboxEmpty", title: `Nothing in ${name}`, hint: "Older mail appears here as the download progresses." };
  };
  const listLoading = () =>
    rows().length === 0 && (threads.loading || (!debounced().trim() && (!!sync()?.foreground || !!sync()?.progress)));

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
        onSignOut={(id) => void removeAccount(id)}
        accounts={accounts()}
        currentAccount={accountView()}
        onSelectAccount={(id) =>
          batch(() => {
            setAccountView(id);
            setSelected(null);
          })
        }
        onAddAccount={() => setOnboarding({ mode: "add" })}
        onPauseHistory={(id, pause) => void pauseHistory(id, pause)}
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
          <Show when={status() && signedIn()}>
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
                  accountId={selectedAccount()}
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
          accounts={accounts()}
          onAddAccount={() => {
            setSettings(false);
            setOnboarding({ mode: "add" });
          }}
          onRemoveAccount={(id) => void removeAccount(id)}
          onSyncWindow={(id, w) => void setSyncWindow(id, w)}
          onAccountNotify={(id, m) => void updateSetting(() => api.setNotifications(m, id))}
          onPauseHistory={(id, pause) => void pauseHistory(id, pause)}
        />
      </Show>
      <Show when={onboarding() && status()}>
        <Onboarding
          mode={onboarding()!.mode}
          resume={onboarding()!.resume}
          clientConfigured={status()!.clientConfigured}
          clientConfigPath={status()!.clientConfigPath}
          onRecheck={() => void refetchStatus()}
          prefs={prefs() ?? status()!.prefs}
          onPref={(k, v) => void setPref(k, v)}
          history={(id) => syncs()[id]?.history ?? null}
          onSignedIn={() => {
            void refetchStatus();
            setTick((n) => n + 1);
          }}
          onClose={() => {
            setOnboarding(null);
            void refetchStatus();
          }}
        />
      </Show>
      <DialogHost />
      <SnackbarHost />
    </div>
  );
}
