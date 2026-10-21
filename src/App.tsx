import { batch, createEffect, createMemo, createResource, createSignal, on, onCleanup, onMount, Show } from "solid-js";
import TopBar from "./components/TopBar";
import Sidebar from "./components/Sidebar";
import ThreadList, { type RowActions } from "./components/ThreadList";
import Reader from "./components/Reader";
import Onboarding from "./components/Onboarding";
import ShortcutsHelp from "./components/ShortcutsHelp";
import CommandPalette from "./components/CommandPalette";
import Compose from "./components/Compose";
import { draftFrom, emptyDraft, withSignature, hasContent, parseMailto } from "./lib/compose";
import type { Command } from "./lib/palette";
import { theme } from "./lib/theme";
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
import { effectivePane, prefs as uiPrefs } from "./lib/prefs";
import { mergeResults, needsServer, serverSearchable } from "./lib/search";
import type {
  Draft,
  SendResult,
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
  const [palette, setPalette] = createSignal(false);
  const [compose, setCompose] = createSignal<{
    key: number;
    title: string;
    initial: Draft;
    accountId: number | null;
    canPick: boolean;
  } | null>(null);
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
  // Gmail server search runs a little after the local one (it costs quota
  // and a round trip) and adds what isn't cached; results merge by thread.
  const [serverQ, setServerQ] = createSignal("");
  createEffect(
    on(debounced, (q) => {
      const h = setTimeout(() => setServerQ(serverSearchable(q) ? q.trim() : ""), 450);
      onCleanup(() => clearTimeout(h));
    }),
  );
  const [serverHits, setServerHits] = createSignal<ThreadRow[]>([]);
  const [serverState, setServerState] = createSignal<"idle" | "loading" | "done" | "error">("idle");
  createEffect(
    on([serverQ, accountView, signedIn], ([q, a, ok]) => {
      setServerHits([]);
      if (!q || !ok) return setServerState("idle");
      let live = true;
      onCleanup(() => (live = false));
      setServerState("loading");
      api
        .searchServer(q, 50, a)
        .then((r) => live && batch(() => (setServerHits(r), setServerState("done"))))
        .catch(() => live && setServerState("error"));
    }),
  );
  const searching = () => !!debounced().trim();
  const loadMore = () => {
    if (!debounced().trim() && !threads.loading && threads().length >= limit()) setLimit((n) => n + PAGE);
  };

  const rows = createMemo(() => {
    const o = overrides();
    const base = searching() ? mergeResults(threads(), serverHits()) : threads();
    return base
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
  // ---- Compose: one docked window at a time; drafts autosave to Gmail.
  let composeKey = 0;
  const defaultAccount = () => accountView() ?? selectedAccount() ?? accounts()[0]?.id ?? null;
  const openCompose = (initial: Draft, title: string, accountId: number | null, canPick: boolean) => {
    if (compose()) {
      snack("Finish or close the message you're writing first.");
      return;
    }
    setCompose({ key: ++composeKey, title, initial, accountId, canPick });
  };
  const signed = (d: Draft, acct: number | null) =>
    withSignature(d, (accounts().find((a) => a.id === acct) ?? accounts()[0])?.signature ?? "");
  const newMessage = (initial: Draft = emptyDraft()) => openCompose(signed(initial, defaultAccount()), "New message", defaultAccount(), true);
  const answer = async (m: Message, mode: "reply" | "reply_all" | "forward") => {
    if (compose()) return snack("Finish or close the message you're writing first.");
    const acct = selectedAccount();
    try {
      const init = await api.composeContext(m.id, mode, acct);
      openCompose(signed(draftFrom(init, mode), acct), mode === "forward" ? "Forward" : mode === "reply_all" ? "Reply all" : "Reply", acct, false);
    } catch (e) {
      snack((e as Error).message);
    }
  };
  const sends = new Map<number, { draft: Draft; accountId: number | null; title: string }>();
  const sendDraft = async (d: Draft, accountId: number | null) => {
    const c = compose();
    setCompose(null);
    try {
      const s = await api.sendMessage(d, accountId);
      const kept = { draft: { ...d, draftId: s.draftId }, accountId, title: c?.title ?? "New message" };
      sends.set(s.sendId, kept);
      if (s.delaySecs > 0)
        snack("Sending…", {
          duration: s.delaySecs * 1000,
          action: {
            label: "Undo",
            run: () =>
              void api.cancelSend(s.sendId).then((undone) => {
                if (!undone) return snack("Already sent");
                sends.delete(s.sendId);
                setCompose({ key: ++composeKey, title: kept.title, initial: kept.draft, accountId, canPick: false });
                snack("Sending undone");
              }),
          },
        });
      else snack("Sending…");
    } catch (e) {
      setCompose({ key: ++composeKey, title: c?.title ?? "New message", initial: d, accountId, canPick: c?.canPick ?? false });
      snack((e as Error).message);
    }
  };
  const closeCompose = (_d: Draft, _a: number | null, saved: boolean) => {
    setCompose(null);
    if (saved) snack("Saved to Drafts");
  };
  const discardCompose = async (d: Draft, accountId: number | null) => {
    setCompose(null);
    if (d.draftId) {
      try {
        await api.deleteDraft(d.draftId, accountId);
      } catch (e) {
        return snack((e as Error).message);
      }
    }
    if (d.draftId || hasContent(d)) snack("Draft discarded");
  };

  const openLink = async (href: string, text: string) => {
    const mail = parseMailto(href);
    if (mail) {
      newMessage(mail);
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
      onEvent<SendResult>("send-result", (r) => {
        const kept = sends.get(r.sendId);
        sends.delete(r.sendId);
        if (r.status === "sent") snack("Message sent");
        else
          snack(`Couldn't send. ${r.message ?? ""} It's saved in Drafts.`, {
            action: kept ? { label: "Open", run: () => openCompose(kept.draft, kept.title, kept.accountId, false) } : undefined,
          });
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

    const step = (d: number) => {
      const i = Math.min(Math.max(cursor() + d, 0), Math.max(0, rows().length - 1));
      setCursor(i);
      if (selected() && layout() === "full" && rows()[i] && rows()[i].threadId !== selected()) openThread(rows()[i], i);
    };
    const withCurrent = (f: (t: ThreadRef) => void) => () => {
      const t = current();
      if (t) f(t);
    };
    const remove = installShortcuts({
      // In full-page reading, j/k open the next/previous conversation (Gmail).
      next: () => step(1),
      prev: () => step(-1),
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
      compose: () => newMessage(),
      reply: (mode) => {
        const last = messages()[messages().length - 1];
        if (selected() && last) void answer(last, mode);
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
    // Ctrl+K opens the palette from anywhere, even while typing.
    const onPaletteKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "k") {
        // In the compose editor Ctrl+K inserts a link.
        if (e.target instanceof Element && e.target.closest(".ProseMirror")) return;
        e.preventDefault();
        if (!onboarding()) setPalette((v) => !v);
      }
    };
    window.addEventListener("keydown", onPaletteKey);
    // Files dropped outside the compose drop zone must never navigate the
    // webview to the file.
    const noDrop = (e: DragEvent) => {
      if (!(e.target instanceof Element && e.target.closest("[data-dropzone]"))) e.preventDefault();
    };
    window.addEventListener("dragover", noDrop);
    window.addEventListener("drop", noDrop);
    onCleanup(() => (window.removeEventListener("dragover", noDrop), window.removeEventListener("drop", noDrop)));
    onCleanup(() => window.removeEventListener("keydown", onPaletteKey));
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
    if (q)
      return {
        icon: "searchOff",
        title: `No results for "${q}"`,
        hint:
          serverState() === "loading"
            ? "Searching all of Gmail…"
            : serverState() === "error"
              ? "Couldn't reach Gmail, so only mail saved on this PC was searched."
              : "Searched Gmail and the mail saved on this PC.",
      };
    if (label() === "INBOX") return { icon: "doneAll", title: "You're all caught up", hint: "New mail shows up here the moment it arrives." };
    const name = labelName(label(), labels().find((l) => l.id === label())?.name ?? label());
    return { icon: "inboxEmpty", title: `Nothing in ${name}`, hint: "Older mail appears here as the download progresses." };
  };
  // Everything the Ctrl+K palette can do, built fresh when it opens.
  const paletteCommands = (): Command[] => {
    const cmds: Command[] = [];
    const t = current();
    const add = (c: Omit<Command, "id"> & { id?: string }) => cmds.push({ id: c.id ?? `${c.group}:${c.title}`, ...c });
    if (t) {
      if (inInbox()) add({ group: "Conversation", title: "Archive", icon: "archive", keys: "e", run: () => archive(t) });
      add({ group: "Conversation", title: t.starred ? "Remove star" : "Star", icon: "star", keys: "s", run: () => toggleStar(t) });
      add({ group: "Conversation", title: t.unread ? "Mark as read" : "Mark as unread", icon: t.unread ? "markRead" : "markUnread", keys: t.unread ? "Shift+I" : "Shift+U", run: () => setRead(t, t.unread) });
      const code = [...messages()].reverse().find((m) => m.otpCode)?.otpCode;
      if (code) add({ group: "Conversation", title: "Copy verification code", icon: "copy", keys: "Shift+C", words: "otp", run: () => void copyCode(code) });
      if (selected()) add({ group: "Conversation", title: "Back to list", icon: "back", keys: "u", run: () => setSelected(null) });
    }
    const system: [string, string, IconName][] = [
      ["INBOX", "Inbox", "inbox"],
      ["STARRED", "Starred", "star"],
      ["IMPORTANT", "Important", "important"],
      ["SENT", "Sent", "send"],
      ["DRAFT", "Drafts", "draft"],
      ["SPAM", "Spam", "spam"],
      ["TRASH", "Trash", "trash"],
    ];
    for (const [id, name, icon] of system) add({ group: "Go to", title: name, icon, words: "go open folder", run: () => goto(id) });
    for (const l of labels())
      if (l.kind === "user") add({ group: "Go to", title: labelName(l.id, l.name), icon: "label", hint: "Label", words: "label", run: () => goto(l.id) });
    if (accounts().length > 1) {
      add({ group: "Accounts", title: "All inboxes", icon: "allInbox", words: "unified switch", run: () => batch(() => (setAccountView(null), setSelected(null))) });
      for (const a of accounts()) add({ group: "Accounts", title: a.email, icon: "person", hint: a.unread ? `${a.unread} unread` : undefined, words: "switch account", run: () => batch(() => (setAccountView(a.id), setSelected(null))) });
    }
    add({ group: "Accounts", title: "Add another account", icon: "add", words: "sign in google", run: () => setOnboarding({ mode: "add" }) });
    add({ group: "Actions", title: "Compose", icon: "pencil", keys: "c", words: "new write email message", run: () => newMessage() });
    if (selected() && messages().length) {
      const last = messages()[messages().length - 1];
      add({ group: "Conversation", title: "Reply", icon: "reply", keys: "r", run: () => void answer(last, "reply") });
      add({ group: "Conversation", title: "Reply all", icon: "replyAll", keys: "a", run: () => void answer(last, "reply_all") });
      add({ group: "Conversation", title: "Forward", icon: "forward", keys: "f", run: () => void answer(last, "forward") });
    }
    add({ group: "Actions", title: "Check mail now", icon: "refresh", keys: "Shift+R", words: "sync refresh", run: () => void api.syncNow() });
    add({ group: "Actions", title: "Search mail", icon: "search", keys: "/", run: () => searchInput?.focus() });
    add({ group: "Layout", title: "Reading pane: right of list", icon: "splitRight", words: "split side", run: () => uiPrefs.setPane("right") });
    add({ group: "Layout", title: "Reading pane: below list", icon: "splitBottom", words: "split bottom", run: () => uiPrefs.setPane("bottom") });
    add({ group: "Layout", title: "Reading pane: full page", icon: "noSplit", words: "no split", run: () => uiPrefs.setPane("full") });
    add({ group: "Layout", title: collapsed() ? "Expand sidebar" : "Collapse sidebar", icon: "menu", run: () => setCollapsed((c) => !c) });
    add({ group: "Appearance", title: "Theme: system", icon: "auto", words: "dark light", run: () => theme.set("system") });
    add({ group: "Appearance", title: "Theme: light", icon: "sun", run: () => theme.set("light") });
    add({ group: "Appearance", title: "Theme: dark", icon: "moon", run: () => theme.set("dark") });
    add({ group: "App", title: "Settings", icon: "settings", keys: ",", words: "preferences options", run: () => setSettings(true) });
    add({ group: "App", title: "Keyboard shortcuts", icon: "keyboard", keys: "?", words: "help", run: () => setHelp(true) });
    return cmds;
  };

  // Reading pane: right / bottom / full page, falling back to full page
  // when the window is too small for the split.
  const [viewport, setViewport] = createSignal({ w: window.innerWidth, h: window.innerHeight });
  {
    const onResize = () => setViewport({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", onResize);
    onCleanup(() => window.removeEventListener("resize", onResize));
  }
  const layout = () => effectivePane(uiPrefs.pane(), viewport().w, viewport().h);
  const listClass = () => {
    if (!selected()) return "min-h-0 flex-1";
    switch (layout()) {
      case "right":
        return "w-[44%] max-w-[640px] shrink-0 border-r border-divider";
      case "bottom":
        return "h-[40%] min-h-[160px] shrink-0";
      default:
        return "hidden";
    }
  };

  const listLoading = () =>
    rows().length === 0 &&
    (threads.loading ||
      (searching() && needsServer(debounced()) && serverState() === "loading") ||
      (!searching() && (!!sync()?.foreground || !!sync()?.progress)));

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
          onCompose={() => newMessage()}
          onSelect={(id) => {
            goto(id);
            void refetchLabels();
          }}
        />
        <main class={`mb-4 mr-4 flex min-w-0 flex-1 overflow-hidden rounded-2xl bg-surface ${layout() === "bottom" ? "flex-col" : ""}`}>
          <Show when={status() && signedIn()}>
            <section class={`thread-list-pane flex min-w-0 flex-col ${listClass()}`}>
              <Show when={searching() && rows().length > 0}>
                <div class="flex h-9 shrink-0 items-center gap-2 border-b border-divider px-4 text-xs text-muted" role="status">
                  <Show
                    when={serverState() === "loading"}
                    fallback={
                      <span>
                        {serverState() === "error"
                          ? "Couldn't reach Gmail · showing mail saved on this PC"
                          : serverState() === "done"
                            ? "Results from Gmail and this PC"
                            : "Results from this PC"}
                      </span>
                    }
                  >
                    <span class="inline-block size-3 animate-spin rounded-full border-2 border-primary border-t-transparent" />
                    <span>Searching all of Gmail…</span>
                  </Show>
                </div>
              </Show>
              <div class="min-h-0 flex-1">
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
              </div>
            </section>
            <Show when={selected()}>
              <section class={`reader-pane min-h-0 min-w-0 flex-1 ${layout() === "bottom" ? "border-t border-divider" : ""}`}>
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
                  onCompose={(m, mode) => void answer(m, mode)}
                  onToggleEmailDark={() => void setPref("email_dark", prefs()?.email_dark === "original" ? "smart" : "original")}
                />
              </section>
            </Show>
          </Show>
        </main>
      </div>
      <Show when={compose()} keyed>
        {(c) => (
          <Compose
            title={c.title}
            initial={c.initial}
            accountId={c.accountId}
            accounts={accounts()}
            canPickAccount={c.canPick}
            onSend={(d, a) => void sendDraft(d, a)}
            onClose={closeCompose}
            onDiscard={(d, a) => void discardCompose(d, a)}
          />
        )}
      </Show>
      <Show when={palette()}>
        <CommandPalette
          commands={paletteCommands()}
          onSearch={(q) => batch(() => (setSelected(null), setQuery(q)))}
          onClose={() => setPalette(false)}
        />
      </Show>
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
          onAccountSettings={(id, st) => void updateSetting(() => api.setAccountSettings(id, st))}
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
