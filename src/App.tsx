import { batch, createEffect, createMemo, createResource, createSignal, lazy, on, onCleanup, onMount, Show, Suspense } from "solid-js";
import TopBar from "./components/TopBar";
import Sidebar from "./components/Sidebar";
import ThreadList, { type RowActions } from "./components/ThreadList";
import SelectionToolbar from "./components/SelectionToolbar";
import { loadRows } from "./lib/paging";
import { emptySelection, selectAll, toggleSelected, isSelected, selectedCount, threadKey, type ThreadKey } from "./lib/selection";
import Reader from "./components/Reader";
import { InboxTabs, CATEGORY_TABS } from "./components/InboxTabs";
import Onboarding from "./components/Onboarding";
import ShortcutsHelp from "./components/ShortcutsHelp";
import CommandPalette from "./components/CommandPalette";
import { draftFrom, draftFromMessage, emptyDraft, withSignature, hasContent, parseMailto } from "./lib/compose";
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
  Category,
  Message,
  NotificationMode,
  OpenTarget,
  PrefKey,
  Prefs,
  SyncStatus,
  SyncWindow,
  ThreadRow,
} from "./lib/types";

type ThreadRef = Pick<ThreadRow, "threadId" | "unread" | "starred" | "important"> & { accountId: number | null };

const VIEW_KEY = "pm.accountView";
// Keep TipTap/ProseMirror out of the inbox startup path.
const Compose = lazy(() => import("./components/Compose"));

/** Optimistic per-thread state shown until the next reload confirms it. */
interface Override {
  unread?: boolean;
  starred?: boolean;
  important?: boolean;
  category?: Category;
  removed?: boolean;
  removedFromCategory?: string;
  removedFromLabel?: string;
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
  const signedIn = createMemo(() => (status()?.accounts.length ?? 0) > 0);
  // Forget a view of an account that was removed.
  createEffect(() => {
    const v = accountView();
    const s = status();
    if (v != null && s && !s.accounts.some((a) => a.id === v)) setAccountView(null);
  });
  createEffect(() => {
    const v = accountView();
    if (v == null) localStorage.removeItem(VIEW_KEY);
    else localStorage.setItem(VIEW_KEY, String(v));
  });
  const inInbox = () => label() === "INBOX" && !debounced().trim();
  // Gmail inbox tabs: on when the viewed account (or any, in All inboxes) wants them.
  const [category, setCategory] = createSignal<Category>("CATEGORY_PERSONAL");
  const tabsOn = createMemo(() => {
    if (!inInbox()) return false;
    const v = accountView();
    const list = status()?.accounts ?? [];
    return list.some((a) => (v == null || a.id === v) && a.categoryTabs !== false);
  });
  const [categoryCounts] = createResource(
    () => (signedIn() && tabsOn() ? `${tick()}|${accountView() ?? ""}` : false),
    () => api.categoryCounts(accountView()),
    { initialValue: [] },
  );

  const [labels, { refetch: refetchLabels }] = createResource(
    () => (signedIn() ? `${tick()}|${accountView() ?? ""}` : false),
    () => api.labels(accountView()),
    { initialValue: [] },
  );
  // Infinite scroll: the list grows a page at a time as you near the end.
  const PAGE = 200;
  const [limit, setLimit] = createSignal(PAGE);
  const pageContexts = new WeakMap<ThreadRow[], string>();
  const [threads] = createResource(
    () =>
      signedIn()
        ? `${label()}|${debounced().trim()}|${tick()}|${limit()}|${accountView() ?? ""}|${tabsOn() ? category() : ""}`
        : false,
    async (sourceKey, info) => {
      if (!sourceKey) return [];
      const [lbl, q, tStr, nStr, aStr, cStr] = sourceKey.split("|");
      const n = Number(nStr);
      const a = aStr ? Number(aStr) : null;
      const c = (cStr || null) as Category | null;
      const context = `${lbl}|${q}|${tStr}|${aStr}|${cStr}`;
      const previous = info.value && pageContexts.get(info.value) === context && info.value.length < n ? info.value : [];
      const result = await loadRows((offset, count) => q ? api.search(q, count, a, offset) : api.threads(lbl, count, offset, a, c), n, previous);
      pageContexts.set(result, context);
      return result;
    },
    { initialValue: [] as ThreadRow[] },
  );
  // Gmail server search runs a little after the local one (it costs quota
  // and a round trip) and adds what isn't cached; results merge by thread.
  const [serverQ, setServerQ] = createSignal("");
  const [serverRetry, setServerRetry] = createSignal(0);
  createEffect(
    on(debounced, (q) => {
      setServerQ("");
      setServerHits([]);
      setServerState("idle");
      const h = setTimeout(() => setServerQ(serverSearchable(q) ? q.trim() : ""), 450);
      onCleanup(() => clearTimeout(h));
    }),
  );
  const [serverHits, setServerHits] = createSignal<ThreadRow[]>([]);
  const [serverState, setServerState] = createSignal<"idle" | "loading" | "done" | "error">("idle");
  createEffect(
    on([serverQ, accountView, signedIn, serverRetry], ([q, a, ok]) => {
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
    if (!threads.loading && threads().length >= limit() && threads().length > 0) setLimit((n) => n + PAGE);
  };

  const rows = createMemo(() => {
    const o = overrides();
    const base = searching() ? mergeResults(threads(), serverHits()) : threads();
    const q = debounced().toLowerCase();
    const unreadOnly = label() === "UNREAD" || q.includes("is:unread");
    const starredOnly = label() === "STARRED" || q.includes("is:starred");
    const curLabel = label();
    const curCat = tabsOn() ? category() : null;

    return base
      .filter((t) => {
        const x = o[t.threadId];
        if (x) {
          if (x.removed) return false;
          if (x.removedFromLabel && x.removedFromLabel === curLabel) return false;
          if (x.removedFromCategory && x.removedFromCategory === curCat) return false;
          const isUnread = x.unread ?? t.unread;
          if (unreadOnly && !isUnread) return false;
          const isStarred = x.starred ?? t.starred;
          if (starredOnly && !isStarred) return false;
          const isImportant = x.important ?? t.important;
          if ((curLabel === "IMPORTANT" || q.includes("is:important")) && !isImportant) return false;
        } else {
          if (unreadOnly && !t.unread) return false;
          if (starredOnly && !t.starred) return false;
          if ((curLabel === "IMPORTANT" || q.includes("is:important")) && !t.important) return false;
        }
        return true;
      })
      .map((t) => {
        const x = o[t.threadId];
        return x
          ? {
              ...t,
              unread: x.unread ?? t.unread,
              starred: x.starred ?? t.starred,
              important: x.important ?? t.important,
            }
          : t;
      });
  });
  const [selection, setSelection] = createSignal(emptySelection());
  const [members, setMembers] = createSignal<ReadonlySet<string>>(new Set());
  const [selectingAll, setSelectingAll] = createSignal(false);
  const [bulkBusy, setBulkBusy] = createSignal(false);
  const [bulkProgress, setBulkProgress] = createSignal("");
  let selectionEpoch = 0;
  let stopBulk = false;
  const selectionCount = () => selectedCount(selection());
  const selectionScope = createMemo(() => JSON.stringify([accountView(), label(), tabsOn() ? category() : null, query().trim()]));
  const selectionReady = () => !threads.loading && query().trim() === debounced().trim() && (!needsServer(debounced()) || serverState() === "done" || serverState() === "error");
  createEffect(on(selectionScope, () => {
    selectionEpoch++;
    stopBulk = true;
    setSelection(emptySelection());
    setMembers(new Set<string>());
    setSelectingAll(false);
    setSelected(null);
  }));
  const selectionKey = (key: string): ThreadKey => {
    const at = key.indexOf(":");
    return { accountId: Number(key.slice(0, at)), threadId: key.slice(at + 1) };
  };
  const checkAll = async () => {
    if (!selectionReady() || selectingAll() || bulkBusy()) return;
    const epoch = ++selectionEpoch;
    setSelectingAll(true);
    try {
      // Unsupported local operators select exactly the loaded Gmail results.
      const source = searching() && needsServer(debounced())
        ? { kind: "threads" as const, threads: rows().map(({ accountId, threadId }) => ({ accountId, threadId })) }
        : { kind: "view" as const, accountId: accountView(), labelId: label(), category: tabsOn() ? category() : null, query: debounced() };
      const all = await api.createSelection(source);
      if (epoch !== selectionEpoch) return;
      setSelection(selectAll(all));
    } catch (e) {
      if (epoch === selectionEpoch) snack((e as Error).message);
    } finally {
      if (epoch === selectionEpoch) setSelectingAll(false);
    }
  };
  // Ask about just the loaded keys; never transfer an entire mailbox to the UI.
  const allSelection = createMemo(() => selection().all);
  createEffect(() => {
    const all = allSelection();
    const keys = rows().map(({ accountId, threadId }) => ({ accountId, threadId }));
    if (!all) return;
    let live = true;
    onCleanup(() => { live = false; });
    const pages: Promise<ThreadKey[]>[] = [];
    for (let i = 0; i < keys.length; i += 5000) pages.push(api.selectionMembers(all.id, keys.slice(i, i + 5000)));
    void Promise.all(pages).then((found) => {
      if (live) setMembers(new Set(found.flat().map(threadKey)));
    }, () => { if (live) setMembers(new Set<string>()); });
  });
  const checked = (t: ThreadRow) => isSelected(selection(), threadKey(t), members().has(threadKey(t)));
  const checkThread = (t: ThreadRow) => {
    if (bulkBusy() || selectingAll() || !selectionReady()) return;
    if (selection().all && !members().has(threadKey(t))) return;
    setSelection((s) => toggleSelected(s, threadKey(t)));
  };
  const clearSelection = () => { selectionEpoch++; setSelection(emptySelection()); setMembers(new Set<string>()); };
  const markSelected = async (targetRead: boolean) => {
    if (bulkBusy() || !selectionCount() || !selectionReady()) return;
    const chosen = selection();
    stopBulk = false;
    setBulkBusy(true);
    setBulkProgress(`Marking selected conversations as ${targetRead ? "read" : "unread"}…`);

    // 1. Instant optimistic update: update all visible matching rows immediately in 0ms!
    const visibleChosen = rows().filter((t) => checked(t));
    const toChange = visibleChosen.filter((t) => (targetRead ? t.unread : !t.unread));
    const optimistic: Record<string, Override> = {};
    for (const t of visibleChosen) {
      optimistic[t.threadId] = { unread: !targetRead };
    }
    setOverrides((o) => ({ ...o, ...optimistic }));

    let changed = 0;
    let failed = 0;
    try {
      if (chosen.all && toChange.length === 0 && rows().length >= 200 && targetRead) {
        // Bulk folder selection with hundreds of unloaded items
        const all = chosen.all;
        const excluded = [...chosen.exceptions].map(selectionKey);
        let offset = 0;
        while (!stopBulk && offset < all.total) {
          const result = await api.markSelectionRead(all.id, offset, offset === 0 ? excluded : []);
          offset = result.nextOffset;
          changed += result.changed;
          failed += result.failed;
          if (result.error || result.done) break;
        }
      } else {
        // Fast targeted one-shot update of the threads that actually need changing
        const targets = toChange.length > 0 ? toChange : visibleChosen;
        const add = targetRead ? [] : ["UNREAD"];
        const remove = targetRead ? ["UNREAD"] : [];
        await Promise.all(
          targets.map((t) =>
            api.modifyThread(t.threadId, add, remove, t.accountId).then(
              () => { changed++; settled.add(t.threadId); },
              () => { failed++; }
            )
          )
        );
      }
      clearSelection();
      snack(`${changed > 0 ? changed.toLocaleString() : "Selected conversations"} marked as ${targetRead ? "read" : "unread"}`);
    } catch (e) {
      failed++;
      snack((e as Error).message);
    } finally {
      setBulkBusy(false);
      setTick((n) => n + 1);
      void refetchLabels();
      void refetchStatus();
    }
  };
  const messages = createMemo(() => {
    const x = selected() ? overrides()[selected()!] : undefined;
    return x
      ? msgs().map((m) => {
          let labelIds = m.labelIds;
          if (x.category) {
            const others = new Set(CATEGORY_TABS.map((c) => c.id));
            labelIds = [...labelIds.filter((l) => !others.has(l as Category)), x.category];
          }
          if (x.important !== undefined) {
            labelIds = x.important
              ? (labelIds.includes("IMPORTANT") ? labelIds : [...labelIds, "IMPORTANT"])
              : labelIds.filter((l) => l !== "IMPORTANT");
          }
          return {
            ...m,
            unread: x.unread ?? m.unread,
            starred: x.starred ?? m.starred,
            labelIds,
          };
        })
      : msgs();
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
      [label, debounced, accountView, category],
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
    if (cached.length) {
      setMsgs((prev) => {
        if (
          prev.length === cached.length &&
          prev.every((p, idx) => p.id === cached[idx].id && p.bodyLoaded === cached[idx].bodyLoaded)
        ) {
          let changed = false;
          const merged = prev.map((p, idx) => {
            const c = cached[idx];
            if (
              p.unread !== c.unread ||
              p.starred !== c.starred ||
              p.labelIds.length !== c.labelIds.length ||
              p.labelIds.some((l, i) => l !== c.labelIds[i])
            ) {
              changed = true;
              return { ...p, unread: c.unread, starred: c.starred, labelIds: c.labelIds };
            }
            return p;
          });
          return changed ? merged : prev;
        }
        return cached;
      });
    }
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
      setBodyError(null);
      if (id) {
        setMsgLoading(true);
        void loadThread(id, true);
      } else {
        setMsgs([]);
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
    // For email_dark, await backend commit first so render_message doesn't race against uncommitted SQLite state.
    if (key === "email_dark") {
      try {
        await api.setPref(key, value);
        setPrefs((p) => (p ? { ...p, [key]: value } : p));
      } catch (e) {
        snack((e as Error).message);
      }
      return;
    }
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
        snack(`Sending in ${s.delaySecs}s…`, {
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
    void modify(t, { removedFromLabel: "INBOX" }, [], ["INBOX"]).then((ok) => {
      if (ok)
        snack("Conversation archived", {
          action: { label: "Undo", run: () => void modify(t, { removedFromLabel: undefined }, ["INBOX"], []) },
        });
    });
  };
  const trash = (t: ThreadRef) => {
    if (selected() === t.threadId) setSelected(null);
    void modify(t, { removedFromLabel: "INBOX", removed: true }, ["TRASH"], ["INBOX"]).then((ok) => {
      if (ok)
        snack("Conversation moved to trash", {
          action: { label: "Undo", run: () => void modify(t, { removed: false }, ["INBOX"], ["TRASH"]) },
        });
    });
  };
  const markSpam = (t: ThreadRef) => {
    if (selected() === t.threadId) setSelected(null);
    void modify(t, { removedFromLabel: "INBOX", removed: true }, ["SPAM"], ["INBOX"]).then((ok) => {
      if (ok)
        snack("Marked as spam", {
          action: { label: "Undo", run: () => void modify(t, { removed: false }, ["INBOX"], ["SPAM"]) },
        });
    });
  };
  const toggleImportant = (t: ThreadRef) => {
    const next = !t.important;
    void modify(t, { important: next }, next ? ["IMPORTANT"] : [], next ? [] : ["IMPORTANT"]).then((ok) => {
      if (ok) snack(next ? "Marked as important" : "Marked as not important");
    });
  };
  const setRead = (t: ThreadRef, read: boolean) => {
    if (!read && selected() === t.threadId) setSelected(null); // Gmail closes on "mark unread"
    void modify(t, { unread: !read }, read ? [] : ["UNREAD"], read ? ["UNREAD"] : []).then((ok) => {
      if (ok) snack(read ? "Marked as read" : "Marked as unread");
    });
  };
  const toggleStar = (t: ThreadRef) => {
    const next = !t.starred;
    void modify(t, { starred: next }, next ? ["STARRED"] : [], next ? [] : ["STARRED"]).then((ok) => {
      if (ok) snack(next ? "Starred conversation" : "Removed star");
    });
  };

  const actions: RowActions = {
    get archive() {
      return inInbox() ? archive : undefined;
    },
    trash,
    markSpam,
    toggleRead: (t) => setRead(t, t.unread),
    toggleStar,
    toggleImportant,
    moveToCategory: (t, c) => void moveToTab(t, c),
  };
  /** The open thread, else the row under the keyboard cursor. */
  const current = (): ThreadRef | undefined => {
    const id = selected();
    if (!id) return rows()[cursor()];
    const row = rows().find((r) => r.threadId === id && r.accountId === (selectedAccount() ?? r.accountId));
    if (row) return row;
    const m = messages();
    return m.length
      ? {
          threadId: id,
          accountId: selectedAccount(),
          unread: m.some((x) => x.unread),
          starred: m.some((x) => x.starred),
          important: m.some((x) => x.labelIds.includes("IMPORTANT")),
        }
      : undefined;
  };

  const editDraftMessage = (m: Message, accountId: number | null) => {
    openCompose(draftFromMessage(m), "Edit draft", accountId, false);
  };

  const openThread = async (t: ThreadRow, index: number) => {
    setCursor(index);
    if (label() === "DRAFT") {
      try {
        const cached = await api.thread(t.threadId, t.accountId);
        const draftMsg = cached.find((m) => m.labelIds.includes("DRAFT") || m.id.startsWith("draft_")) ?? cached[cached.length - 1];
        if (draftMsg) {
          editDraftMessage(draftMsg, t.accountId);
          return;
        }
      } catch {
        // Fallback to opening thread
      }
    }
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

  // Move a thread to another Gmail inbox tab (CATEGORY_* labels).
  const moveToTab = async (t: ThreadRef, c: Category) => {
    const others = CATEGORY_TABS.map((x) => x.id).filter((x) => x !== c);
    const prev = overrides()[t.threadId];
    const sourceCategory = category();
    // Real-time optimistic removal from the current category view and update category metadata
    setOverrides((o) => ({
      ...o,
      [t.threadId]: {
        ...o[t.threadId],
        removedFromCategory: sourceCategory,
        category: c,
      },
    }));
    // Note: Do NOT close reader when moving categories; keep reading uninterrupted
    try {
      await api.modifyThread(t.threadId, [c], others, t.accountId);
      settled.add(t.threadId);
      snack(`Moved to ${CATEGORY_TABS.find((x) => x.id === c)?.name ?? "tab"}`, {
        action: {
          label: "Undo",
          run: () => {
            setOverrides((o) => {
              const n = { ...o };
              if (prev) n[t.threadId] = prev;
              else delete n[t.threadId];
              return n;
            });
            void api.modifyThread(t.threadId, [sourceCategory], [c], t.accountId);
          },
        },
      });
    } catch (e) {
      setOverrides((o) => {
        const n = { ...o };
        if (prev) n[t.threadId] = prev;
        else delete n[t.threadId];
        return n;
      });
      snack((e as Error).message);
    }
  };

  const quickSend = async (m: Message, text: string, mode: "reply" | "reply_all") => {
    const acct = selectedAccount();
    try {
      const init = await api.composeContext(m.id, mode, acct);
      const d = signed(draftFrom(init, mode), acct);
      d.body = text;
      d.html = `<p>${text.replace(/\n/g, "<br>")}</p>`;
      await sendDraft(d, acct);
    } catch (e) {
      snack((e as Error).message);
    }
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
    let mailTimer: ReturnType<typeof setTimeout> | undefined;
    void Promise.all([
      onEvent<null>("mail-updated", () => {
        clearTimeout(mailTimer);
        mailTimer = setTimeout(() => setTick((n) => n + 1), 350);
      }),
      onEvent<null>("accounts-changed", () => void refetchStatus()),
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
      clearTimeout(mailTimer);
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
      trash: withCurrent(trash),
      markSpam: withCurrent(markSpam),
      star: withCurrent(toggleStar),
      markRead: () => selectionCount() ? void markSelected(true) : withCurrent((t) => setRead(t, true))(),
      markUnread: () => selectionCount() ? void markSelected(false) : withCurrent((t) => setRead(t, false))(),
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
              : serverState() === "idle"
                ? "Searched the mail saved on this PC. Type at least two characters to also search Gmail."
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
      if (tabsOn())
        for (const c of CATEGORY_TABS)
          if (c.id !== category())
            add({ group: "Conversation", title: `Move to ${c.name}`, icon: c.icon, words: "tab category move", run: () => void moveToTab(t, c.id) });
    }
    if (tabsOn())
      for (const c of CATEGORY_TABS)
        add({ group: "Go to", title: `${c.name} tab`, icon: c.icon, words: "tab category inbox", run: () => batch(() => (setCategory(c.id), setSelected(null))) });
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
    add({ group: "Layout", title: "Open emails: side pane", icon: "splitRight", words: "reading pane split right list", run: () => uiPrefs.setPane("right") });
    add({ group: "Layout", title: "Open emails: below list", icon: "splitBottom", words: "reading pane split bottom", run: () => uiPrefs.setPane("bottom") });
    add({ group: "Layout", title: "Open emails: full view", icon: "noSplit", words: "reading pane full page no split gmail", run: () => uiPrefs.setPane("full") });
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
  const [resizing, setResizing] = createSignal(false);

  const startResizeWidth = (e: PointerEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startW = uiPrefs.splitWidth();
    setResizing(true);

    const onMove = (ev: PointerEvent) => {
      const delta = ev.clientX - startX;
      const maxW = Math.max(300, Math.floor(window.innerWidth * 0.65));
      const next = Math.min(Math.max(startW + delta, 260), maxW);
      uiPrefs.setSplitWidth(next);
    };

    const onUp = () => {
      setResizing(false);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  const startResizeHeight = (e: PointerEvent) => {
    e.preventDefault();
    const startY = e.clientY;
    const startH = uiPrefs.splitHeight();
    setResizing(true);

    const onMove = (ev: PointerEvent) => {
      const delta = ev.clientY - startY;
      const maxH = Math.max(200, Math.floor(window.innerHeight * 0.65));
      const next = Math.min(Math.max(startH + delta, 160), maxH);
      uiPrefs.setSplitHeight(next);
    };

    const onUp = () => {
      setResizing(false);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  const listClass = () => {
    if (!selected()) return "min-h-0 flex-1";
    switch (layout()) {
      case "right":
        return "shrink-0";
      case "bottom":
        return "shrink-0";
      default:
        return "hidden";
    }
  };

  const listStyle = () => {
    if (!selected()) return {};
    if (layout() === "right") return { width: `${uiPrefs.splitWidth()}px`, "max-width": "65vw" };
    if (layout() === "bottom") return { height: `${uiPrefs.splitHeight()}px`, "max-height": "65vh" };
    return {};
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
          categoryCounts={categoryCounts()}
          current={label()}
          category={tabsOn() ? category() : null}
          collapsed={collapsed()}
          onCompose={() => newMessage()}
          onSelect={(id) => {
            goto(id);
            void refetchLabels();
          }}
          onSelectCategory={(c) => {
            batch(() => {
              setQuery("");
              setSelected(null);
              setLabel("INBOX");
              setCategory(c);
            });
            void refetchLabels();
          }}
        />
        <main class={`mb-4 mr-4 flex min-w-0 flex-1 overflow-hidden rounded-2xl bg-surface ${layout() === "bottom" ? "flex-col" : ""} ${resizing() ? "select-none" : ""}`}>
          <Show when={status() && signedIn()}>
            <section class={`thread-list-pane flex min-w-0 flex-col ${listClass()}`} style={listStyle()}>
              <SelectionToolbar
                count={selectionCount()} all={!!selection().all && selection().exceptions.size === 0}
                disabled={!selectionReady()} selecting={selectingAll()} busy={bulkBusy()} progress={bulkProgress()}
                scope={selection().all
                  ? searching() && needsServer(debounced()) ? "Selection covers loaded Gmail results only." : "Selection includes matching mail saved on this PC, including unloaded rows. New arrivals are excluded."
                  : "Select all matching conversations saved on this PC, including unloaded rows"}
                onSelectAll={() => void checkAll()} onClear={clearSelection} onRead={() => void markSelected(true)} onUnread={() => void markSelected(false)}
                onStop={() => { stopBulk = true; setBulkProgress("Stopping after this batch…"); }}
              />
              <Show when={searching()}>
                <div class="flex min-h-9 shrink-0 flex-wrap items-center gap-2 border-b border-divider px-4 py-1 text-xs text-muted" role="status">
                  <Show
                    when={serverState() === "loading"}
                    fallback={
                      <span>
                        {serverState() === "error"
                          ? "Couldn't reach Gmail · showing mail saved on this PC"
                          : serverState() === "done"
                            ? `${rows().length.toLocaleString()} loaded results from Gmail and this PC`
                            : "Results from this PC"}
                      </span>
                    }
                  >
                    <span class="inline-block size-3 animate-spin rounded-full border-2 border-primary border-t-transparent" />
                    <span>Searching all of Gmail…</span>
                  </Show>
                  <Show when={serverState() === "error"}>
                    <button class="rounded-full px-2 py-1 font-medium text-primary hover:bg-hover" onClick={() => setServerRetry((n) => n + 1)}>Retry Gmail search</button>
                  </Show>
                </div>
              </Show>
              <Show when={tabsOn()}>
                <InboxTabs
                  counts={categoryCounts()}
                  current={category()}
                  onSelect={(c) =>
                    batch(() => {
                      setCategory(c);
                      setSelected(null);
                      setLimit(PAGE);
                    })
                  }
                />
              </Show>
              <div class="min-h-0 flex-1">
              <ThreadList
                threads={rows()}
                selectedId={selected()}
                cursor={cursor()}
                loading={listLoading()}
                empty={empty()}
                actions={actions}
                isChecked={checked}
                onCheck={checkThread}
                selectionDisabled={bulkBusy() || selectingAll() || !selectionReady()}
                viewKey={selectionScope()}
                inInbox={inInbox()}
                onOpen={openThread}
                onEndReached={loadMore}
                onCopyCode={(c) => void copyCode(c)}
              />
              </div>
            </section>
            <Show when={selected() && layout() === "right"}>
              <div
                class="group relative z-20 flex w-2 -mx-1 shrink-0 cursor-col-resize select-none items-center justify-center transition-colors hover:bg-primary/20 active:bg-primary/30"
                onPointerDown={startResizeWidth}
                onDblClick={() => uiPrefs.setSplitWidth(420)}
                title="Drag to resize panels (Double-click to reset)"
              >
                <div class="h-8 w-1 rounded-full bg-outline/30 group-hover:bg-primary transition-colors" />
              </div>
            </Show>
            <Show when={selected() && layout() === "bottom"}>
              <div
                class="group relative z-20 flex h-2 -my-1 shrink-0 cursor-row-resize select-none items-center justify-center transition-colors hover:bg-primary/20 active:bg-primary/30"
                onPointerDown={startResizeHeight}
                onDblClick={() => uiPrefs.setSplitHeight(280)}
                title="Drag to resize panels (Double-click to reset)"
              >
                <div class="h-1 w-8 rounded-full bg-outline/30 group-hover:bg-primary transition-colors" />
              </div>
            </Show>
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
                  onOtpChanged={() => {
                    const id = selected();
                    if (id) void loadThread(id, true);
                  }}
                  onArchive={
                    inInbox()
                      ? () => {
                          const t = current();
                          if (t) archive(t);
                        }
                      : undefined
                  }
                  onTrash={() => {
                    const t = current();
                    if (t) trash(t);
                  }}
                  onSpam={() => {
                    const t = current();
                    if (t) markSpam(t);
                  }}
                  onToggleRead={() => {
                    const t = current();
                    if (t) setRead(t, t.unread);
                  }}
                  onToggleStar={() => {
                    const t = current();
                    if (t) toggleStar(t);
                  }}
                  onToggleImportant={() => {
                    const t = current();
                    if (t) toggleImportant(t);
                  }}
                  onLink={(href, text) => void openLink(href, text)}
                  onAttachment={(m, a, open) => void attachment(m, a, open)}
                  emailDark={prefs()?.email_dark ?? "smart"}
                  onCompose={(m, mode) => void answer(m, mode)}
                  onEditDraft={(m) => editDraftMessage(m, selectedAccount())}
                  onMoveCategory={(c) => {
                    const t = current();
                    if (t) void moveToTab(t, c);
                  }}
                  onQuickSend={quickSend}
                  onToggleEmailDark={async () => {
                    const cur = prefs()?.email_dark ?? "smart";
                    const next = cur === "original" ? "smart" : "original";
                    await setPref("email_dark", next);
                  }}
                />
              </section>
            </Show>
          </Show>
        </main>
      </div>
      <Show when={compose()} keyed>
        {(c) => (
          <Suspense fallback={<div role="status" class="fixed bottom-0 right-5 z-40 flex h-16 max-w-full items-center rounded-t-2xl bg-surface-high px-5 text-sm text-muted shadow-2xl">Opening message…</div>}>
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
          </Suspense>
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
