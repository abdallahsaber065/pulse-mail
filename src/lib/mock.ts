// Demo mailbox for UI work (`pnpm dev`) and optional Tauri `PULSE_MAIL_DEMO=1`.
// Never used in release builds. All senders and companies are fictional.
import type { Account, Activity, AppStatus, Draft, Label, Message, RenderedMessage, SyncStatus, ThreadRow } from "./types";
import { threadKey, type SelectionSource, type ThreadKey } from "./selection";
import { needsServer } from "./search";

const now = Date.now();
const min = 60_000;

/** Stand-in OTP toast / first inbox row. Fictional sender, RFC 2606 domain. */
export const DEMO = {
  email: "alex@example.com",
  displayName: "Alex Rivera",
  otpCode: "482917",
  otpThreadId: "t0",
  otpFrom: "Northwind Auth",
  otpSubject: "Your Northwind sign-in code",
} as const;

/** Fictional draft opened by `?compose=1` in demo mode. */
export const DEMO_COMPOSE: Draft = {
  to: "mira@lumenforge.example",
  cc: "jules@brightmill.example",
  bcc: "",
  subject: "Thursday design review — density notes",
  body: "Hi Mira,\n\nThursday at 3pm works. I'll bring the density notes (compact 32 / default 40 / comfortable 52) and a couple of screenshots from the mock mailbox.\n\nNothing in this draft is real mail.\n\nAlex",
  quote: null,
  inReplyTo: "",
  references: "",
  threadId: null,
  draftId: "r-mock-draft",
  html: null,
  attachments: [{ name: "density-notes.txt", mime: "text/plain", size: 2048, staged: "s-demo" }],
};

type Seed = {
  fromName: string;
  fromEmail: string;
  subject: string;
  snippet: string;
  otp: string | null;
  labels: string[];
  unread: boolean;
  starred: boolean;
  important: boolean;
  attachments?: boolean;
  unsub?: boolean;
  extraMessages?: number;
};

const seeds: Seed[] = [
  {
    fromName: DEMO.otpFrom,
    fromEmail: "noreply@auth.northwind.example",
    subject: DEMO.otpSubject,
    snippet: `Verification code: ${DEMO.otpCode}. It expires in 10 minutes. Don't share this code.`,
    otp: DEMO.otpCode,
    labels: ["INBOX"],
    unread: true,
    starred: false,
    important: true,
  },
  {
    fromName: "Mira Okonkwo",
    fromEmail: "mira@lumenforge.example",
    subject: "Design review Thursday",
    snippet: "Can we push the review to 3pm? I want to walk through the new sidebar density before we lock type sizes.",
    otp: null,
    labels: ["INBOX", "Label_1"],
    unread: true,
    starred: false,
    important: false,
  },
  {
    fromName: "Harbor Ledger",
    fromEmail: "receipts@harborledger.example",
    subject: "Invoice HL-4418 is ready",
    snippet: "Thanks for your business. Invoice HL-4418 for $128.00 is ready to download.",
    otp: null,
    labels: ["INBOX", "Label_2"],
    unread: true,
    starred: false,
    important: false,
    unsub: true,
  },
  {
    fromName: "Jules Park",
    fromEmail: "jules@brightmill.example",
    subject: "Q4 notes + the spreadsheet",
    snippet: "Hi Alex, attached is the planning sheet and a short architecture note. Glance at the latency tab if you have a minute.",
    otp: null,
    labels: ["INBOX", "Label_1"],
    unread: false,
    starred: true,
    important: true,
    attachments: true,
    extraMessages: 2,
  },
  {
    fromName: "Pinnacle Cloud",
    fromEmail: "notify@pinnaclecloud.example",
    subject: "Staging deploy finished",
    snippet: "pulse-mail-demo / staging is live. Preview it when you have a moment.",
    otp: null,
    labels: ["INBOX"],
    unread: false,
    starred: false,
    important: false,
    unsub: true,
  },
  {
    fromName: "Riverstone Labs",
    fromEmail: "security@riverstone.example",
    subject: "New sign-in on this device",
    snippet: "We noticed a sign-in from Pulse Mail on Linux. If this was you, you can ignore this message.",
    otp: null,
    labels: ["INBOX"],
    unread: false,
    starred: false,
    important: false,
  },
  {
    fromName: "The Copper Post",
    fromEmail: "editor@copperpost.example",
    subject: "Weekly digest: 7 items",
    snippet: "This week: local-first mail clients, IMAP IDLE explained, and a short note on OTP toast UX.",
    otp: null,
    labels: ["INBOX"],
    unread: false,
    starred: false,
    important: false,
    unsub: true,
  },
  {
    fromName: "Sam Ellison",
    fromEmail: "sam@example.com",
    subject: "Weekend hike?",
    snippet: "Still on for Saturday morning? I'll bring the thermos. Reply if the weather looks grim.",
    otp: null,
    labels: ["INBOX"],
    unread: false,
    starred: false,
    important: false,
  },
];

const extraLabels: Record<string, string[]> = {};
const extraFlags: Record<string, { attachments: boolean; unsub: boolean }> = {};

const threads: ThreadRow[] = seeds.map((s, i) => {
  const threadId = `t${i}`;
  extraLabels[threadId] = s.labels;
  extraFlags[threadId] = { attachments: !!s.attachments, unsub: !!s.unsub };
  return {
    accountId: 1,
    threadId,
    lastMessageId: `m${i}`,
    subject: s.subject,
    fromName: s.fromName,
    fromEmail: s.fromEmail,
    snippet: s.snippet,
    internalDate: now - i * i * 37 * min,
    messageCount: 1 + (s.extraMessages ?? 0),
    unread: s.unread,
    starred: s.starred,
    otpCode: s.otp,
    important: s.important,
  };
});

const CATS = ["CATEGORY_PERSONAL", "CATEGORY_UPDATES", "CATEGORY_PROMOTIONS", "CATEGORY_SOCIAL"] as const;
const catOf = (t: ThreadRow) =>
  t.otpCode || t.threadId === "t1" || t.threadId === "t3" ? "CATEGORY_PERSONAL" : CATS[Number(t.threadId.slice(1)) % CATS.length];

function labelsOf(t: ThreadRow): string[] {
  return extraLabels[t.threadId] ?? ["INBOX"];
}

function countLabel(id: string): { unread: number; total: number } {
  const ts = threads.filter((t) => labelsOf(t).includes(id) || (id === "STARRED" && t.starred));
  return { unread: ts.filter((t) => t.unread).length, total: ts.length };
}

function labels(): Label[] {
  const inbox = countLabel("INBOX");
  const starred = countLabel("STARRED");
  const clients = countLabel("Label_1");
  const receipts = countLabel("Label_2");
  return [
    { id: "INBOX", name: "INBOX", kind: "system", unread: inbox.unread, total: inbox.total },
    { id: "STARRED", name: "STARRED", kind: "system", unread: starred.unread, total: starred.total },
    { id: "SENT", name: "SENT", kind: "system", unread: 0, total: 0 },
    { id: "DRAFT", name: "DRAFT", kind: "system", unread: 0, total: 0 },
    { id: "Label_1", name: "Clients", kind: "user", unread: clients.unread, total: clients.total },
    { id: "Label_2", name: "Receipts", kind: "user", unread: receipts.unread, total: receipts.total },
  ];
}

function message(t: ThreadRow): Message {
  const flags = extraFlags[t.threadId] ?? { attachments: false, unsub: false };
  const html = t.otpCode
    ? `<div style="font-family:Segoe UI,Arial,sans-serif;max-width:520px;margin:24px auto;padding:24px;border:1px solid #333;border-radius:12px;background:#18181b;color:#f4f4f5"><h2 style="margin:0 0 12px;color:#fff">Verify your sign-in</h2><p style="color:#a1a1aa">Use this verification code to securely access your Northwind account:</p><p style="font-size:36px;letter-spacing:8px;font-weight:700;color:#38bdf8;margin:20px 0;font-family:monospace">${t.otpCode}</p><p style="color:#71717a;font-size:12px">If you didn't request this code, you can safely disregard this email. <a href="https://help.northwind.example" style="color:#38bdf8">Help Center</a></p><img src="https://tracker.example.com/pixel.gif" width="1" height="1"></div>`
    : t.threadId === "t3"
    ? `<div style="font-family:Segoe UI,-apple-system,BlinkMacSystemFont,Roboto,sans-serif;max-width:680px;line-height:1.6;color:#e4e4e7">
        <p style="margin-top:0">Hi Alex,</p>
        <p>Attached is the planning sheet and a short architecture note for the fictional Brightmill Q4 cycle. Nothing here is real mail — it's demo data for screenshots.</p>
        <div style="background:#18181b;border:1px solid #27272a;border-radius:12px;padding:18px;margin:20px 0">
          <h4 style="margin:0 0 10px;color:#fafafa;font-size:14px">Attachments</h4>
          <ul style="margin:0;padding-left:20px;color:#a1a1aa;font-size:13px;line-height:1.7">
            <li><strong style="color:#f4f4f5">Q4-Planning.xlsx:</strong> milestones and a latency budget tab.</li>
            <li><strong style="color:#f4f4f5">Architecture-Note.pdf:</strong> a one-pager on local cache + IMAP push.</li>
            <li><strong style="color:#f4f4f5">System-Diagram.png:</strong> boxes and arrows, nothing confidential.</li>
          </ul>
        </div>
        <p>Glance at the spreadsheet in the in-app previewer if you want to see that path.</p>
        <p style="margin-top:28px">Thanks,<br><strong style="color:#fff">Jules Park</strong><br><span style="color:#71717a;font-size:12px">Brightmill · made-up company</span></p>
      </div>`
    : null;
  return {
    id: t.lastMessageId,
    threadId: t.threadId,
    internalDate: t.internalDate,
    fromName: t.fromName,
    fromEmail: t.fromEmail,
    toList: DEMO.email,
    subject: t.subject,
    snippet: t.snippet,
    bodyText: `${t.snippet}\n\nThanks,\n${t.fromName}`,
    bodyHtml: html,
    labelIds: labelsOf(t),
    unread: t.unread,
    starred: t.starred,
    hasAttachments: flags.attachments,
    otpCode: t.otpCode,
    bodyLoaded: true,
    ccList: "",
    replyTo: "",
    rfcMessageId: `<${t.lastMessageId}@demo.example>`,
    rfcReferences: "",
    listUnsubscribe: flags.unsub
      ? `<https://unsubscribe.example/u?id=${t.threadId}>, <mailto:unsub@example.com>`
      : null,
    attachments: flags.attachments
      ? [
          { part: 0, filename: "Q4-Planning.xlsx", mime: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", size: 24_576, inline: false },
          { part: 1, filename: "Architecture-Note.pdf", mime: "application/pdf", size: 482_133, inline: false },
          { part: 2, filename: "System-Diagram.png", mime: "image/png", size: 108_420, inline: false },
        ]
      : [],
  };
}

function syncOf(id: number, email: string): SyncStatus {
  return {
    accountId: id,
    email,
    running: true,
    imapConnected: true,
    syncing: false,
    foreground: false,
    lastSyncAt: now,
    error: null,
    progress: null,
    backfill: null,
    history: { state: "done", window: "90d", phase: "", done: 4210, total: 4210, rate: 0, etaSecs: null, error: null, bytesTotal: 512e6, bytesText: 96e6 },
  };
}

function accountOf(id: number, email: string): Account {
  return {
    id,
    email,
    historyId: "1",
    lastSyncAt: now,
    syncWindow: "90d",
    notify: "all",
    backfillState: "done",
    backfillDone: 4210,
    backfillTotal: 4210,
    messagesTotal: 48_312,
    backfillPhase: "",
    backfillError: null,
    attachMode: "limit",
    attachMaxMb: 10,
    displayName: email === DEMO.email ? DEMO.displayName : "",
    avatarData: null,
    color: "",
    signature: email === DEMO.email ? "Alex Rivera\nLumenforge (demo mailbox)" : "",
    categoryTabs: false,
    sync: syncOf(id, email),
    unread: threads.filter((t) => t.unread && t.accountId === id).length,
  };
}

// `?onboarding` in the URL starts with no account (first-run flow).
const firstRun = typeof location !== "undefined" && location.search.includes("onboarding");

const status: AppStatus = {
  clientConfigured: true,
  clientConfigPath: "%APPDATA%\\com.abdallah.pulsemail\\google-oauth-client.json",
  account: firstRun ? null : DEMO.email,
  accounts: firstRun ? [] : [accountOf(1, DEMO.email)],
  dbBytes: 182_000_000,
  destroyOnClose: false,
  autostart: true,
  notifications: "all",
  dataDir: "%APPDATA%\\com.abdallah.pulsemail",
  version: "0.4.1",
  prefs: {
    images: "ask",
    email_dark: "smart",
    otp_autocopy: "0",
    clipboard_clear_secs: "60",
    otp_lockscreen: "hide",
    undo_send_secs: "5",
    storage_budget_mb: "1024",
    quiet_start: "off",
    quiet_end: "off",
    quiet_allow_otp: "1",
    update_check: "1",
    update_auto_download: "1",
  },
};

function mockRender(m: Message, dark: boolean): RenderedMessage {
  const esc = (s: string) => s.replace(/[&<>"]/g, (c) => `&#${c.charCodeAt(0)};`);
  const color = dark ? "#e4e4e7" : "#18181b";
  const bg = dark ? "transparent" : "#ffffff";
  const body = m.bodyHtml ?? `<div style="white-space:pre-wrap">${esc(m.bodyText)}</div>`;
  return {
    srcdoc: `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:"><style>html,body{background:${bg}!important;color:${color}!important;margin:0;padding:2px 0;font:14px/1.6 system-ui,-apple-system,sans-serif;color-scheme:${dark ? "dark" : "light"};overflow-wrap:anywhere}a{color:${dark ? "#38bdf8" : "#0284c7"}}</style></head><body dir="auto">${body}</body></html>`,
    remoteImages: m.bodyHtml ? 1 : 0,
    imagesShown: 0,
    trackers: m.bodyHtml ? 1 : 0,
    plainText: !m.bodyHtml,
    sender: m.fromEmail,
    senderAllowed: false,
  };
}

const activity: Activity[] = [
  { at: now - 60_000, account: DEMO.email, level: "success", text: "Inbox ready: 50 newest messages in 1.0 s" },
  { at: now - 30_000, account: DEMO.email, level: "info", text: "Live: Gmail will push new mail instantly" },
];

let selectionId = 0;
let selectedKeys: ThreadKey[] = [];
let excludedKeys = new Set<string>();

function inView(t: ThreadRow, labelId: string, category: unknown): boolean {
  if (labelId === "STARRED") return t.starred;
  if (labelId === "SENT" || labelId === "DRAFT" || labelId === "SPAM" || labelId === "TRASH") return false;
  if (!labelsOf(t).includes(labelId)) return false;
  if (category) return catOf(t) === category;
  return true;
}

function mockSearch(query: string) {
  const tokens = query.match(/(?:[^\s"]+|"[^"]*")+/g) ?? [];
  return threads.filter((t) => tokens.every((raw) => {
    const token = raw.toLowerCase();
    if (token === "is:unread") return t.unread;
    if (token === "is:read") return !t.unread;
    if (token === "is:starred") return t.starred;
    if (token === "has:attachment") return extraFlags[t.threadId]?.attachments === true;
    if (token.startsWith("from:")) return `${t.fromName} ${t.fromEmail}`.toLowerCase().includes(token.slice(5).replace(/"/g, ""));
    if (token.startsWith("subject:")) return t.subject.toLowerCase().includes(token.slice(8).replace(/"/g, ""));
    return `${t.subject} ${t.fromName} ${t.snippet}`.toLowerCase().includes(token.replace(/"/g, ""));
  }));
}

export async function handle(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  switch (cmd) {
    case "get_status":
      return structuredClone(status);
    case "create_selection": {
      const source = args?.source as SelectionSource;
      const candidates = source.kind === "threads" ? source.threads : threads.filter((t) =>
        (source.accountId == null || source.accountId === t.accountId) &&
        (source.query ? mockSearch(source.query).includes(t) :
          inView(t, source.labelId ?? "INBOX", source.category)));
      selectedKeys = candidates.map(({ accountId, threadId }) => ({ accountId, threadId }));
      return { id: ++selectionId, total: selectedKeys.length };
    }
    case "selection_members": {
      if (args?.selectionId !== selectionId) throw new Error("This selection has changed.");
      const keys = new Set(selectedKeys.map(threadKey));
      return (args?.threads as ThreadKey[]).filter((k) => keys.has(threadKey(k)));
    }
    case "mark_selection_read": {
      if (args?.selectionId !== selectionId) throw new Error("This selection has changed.");
      const offset = Number(args?.offset ?? 0);
      if (offset === 0) excludedKeys = new Set((args?.excluded as ThreadKey[]).map(threadKey));
      const keys = selectedKeys.slice(offset, offset + 10);
      let changed = 0;
      for (const key of keys) {
        const t = threads.find((t) => threadKey(t) === threadKey(key));
        if (t?.unread && !excludedKeys.has(threadKey(key))) { t.unread = false; changed++; }
      }
      await new Promise((r) => setTimeout(r, 250));
      return { nextOffset: offset + keys.length, done: offset + keys.length >= selectedKeys.length, changed, failed: 0, error: null };
    }
    case "list_labels":
      return labels();
    case "list_threads": {
      const matches = threads.filter((t) => inView(t, String(args?.labelId ?? "INBOX"), args?.category ?? null));
      const offset = Number(args?.offset ?? 0);
      return matches.slice(offset, offset + Number(args?.limit ?? 200));
    }
    case "category_counts":
      return ["CATEGORY_PERSONAL", "CATEGORY_PROMOTIONS", "CATEGORY_SOCIAL", "CATEGORY_UPDATES", "CATEGORY_FORUMS"].map((id) => {
        const ts = threads.filter((t) => catOf(t) === id);
        return { id, unread: ts.filter((t) => t.unread).length, total: ts.length };
      });
    case "otp_feedback":
      if (!args?.useful) for (const t of threads) if (t.lastMessageId === args?.messageId) t.otpCode = null;
      return null;
    case "search": {
      const offset = Number(args?.offset ?? 0);
      return mockSearch(String(args?.query ?? "")).slice(offset, offset + Number(args?.limit ?? 100));
    }
    case "compose_context": {
      const t = threads.find((x) => x.lastMessageId === args?.messageId) ?? threads[0];
      const mode = args?.mode;
      return {
        to: mode === "forward" ? "" : `"${t.fromName}" <${t.fromEmail}>`,
        cc: mode === "reply_all" ? "team@example.com" : "",
        subject: `${mode === "forward" ? "Fwd" : "Re"}: ${t.subject}`,
        inReplyTo: "<mock@example.com>",
        references: "<mock@example.com>",
        threadId: mode === "forward" ? null : t.threadId,
        originalFrom: `"${t.fromName}" <${t.fromEmail}>`,
        originalTo: DEMO.email,
        originalCc: "",
        originalDate: t.internalDate,
        originalSubject: t.subject,
        originalText: t.snippet,
        attachments: mode === "forward" ? [{ name: "report.pdf", mime: "application/pdf", size: 240_000, sourceMessage: t.lastMessageId, sourcePart: 0 }] : [],
      };
    }
    case "save_draft":
      await new Promise((r) => setTimeout(r, 300));
      return { draftId: (args?.draft as { draftId?: string })?.draftId ?? "r-mock-draft", threadId: null };
    case "delete_draft":
      return null;
    case "send_message":
      return { sendId: 1, draftId: "r-mock-draft", delaySecs: 5 };
    case "cancel_send":
      return true;
    case "search_server": {
      await new Promise((r) => setTimeout(r, 700));
      if (!needsServer(String(args?.query ?? ""))) return mockSearch(String(args?.query ?? ""));
      const q = String(args?.query ?? "").toLowerCase().replace(/\b\w+:/g, "");
      const older: ThreadRow = {
        ...threads[threads.length - 1],
        threadId: "srv-old-1",
        lastMessageId: "srv-old-1",
        subject: `Archived: ${q.trim() || "result"} (from Gmail)`,
        snippet: "An older message found by Gmail search",
        internalDate: Date.now() - 400 * 86_400_000,
        unread: false,
      };
      return [...threads.filter((t) => `${t.subject} ${t.fromName} ${t.snippet}`.toLowerCase().includes(q.trim())), older];
    }
    case "get_activity":
      return activity;
    case "modify_thread": {
      const t = threads.find((x) => x.threadId === args?.threadId);
      const add = (args?.add as string[]) ?? [];
      const remove = (args?.remove as string[]) ?? [];
      if (t) {
        if (add.includes("STARRED")) t.starred = true;
        if (remove.includes("STARRED")) t.starred = false;
        if (add.includes("UNREAD")) t.unread = true;
        if (remove.includes("UNREAD")) t.unread = false;
        if (remove.includes("INBOX")) {
          extraLabels[t.threadId] = (extraLabels[t.threadId] ?? []).filter((l) => l !== "INBOX");
          threads.splice(threads.indexOf(t), 1);
        }
      }
      return;
    }
    case "set_notifications":
      status.notifications = args?.mode as AppStatus["notifications"];
      return;
    case "set_destroy_on_close":
      status.destroyOnClose = Boolean(args?.enabled);
      return;
    case "load_thread":
    case "get_thread": {
      const t = threads.find((x) => x.threadId === args?.threadId);
      return t ? [message(t)] : [];
    }
    case "render_message": {
      const t = threads.find((x) => x.lastMessageId === args?.messageId);
      return t ? mockRender(message(t), Boolean(args?.dark)) : null;
    }
    case "set_pref":
      (status.prefs as unknown as Record<string, string>)[String(args?.key)] = String(args?.value);
      return;
    case "save_attachment":
      return { path: "/tmp/Q4-Planning.xlsx", opened: Boolean(args?.open), blocked: false };
    case "take_pending_open":
      return null;
    case "copy_text":
      await navigator.clipboard?.writeText(String(args?.text ?? ""));
      return;
    case "open_external":
      window.open(String(args?.url), "_blank", "noopener");
      return;
    case "open_logs_dir":
      console.info("[demo] would open the logs folder");
      return;
    case "update_check":
      return null;
    case "update_install":
      return;
    case "sign_in": {
      const reconnectId = args?.accountId;
      if (reconnectId != null) {
        const a = status.accounts.find((a) => a.id === reconnectId);
        if (!a) throw new Error("Account not found.");
        return { accountId: a.id, email: a.email, messagesTotal: a.messagesTotal, threadsTotal: 31_020, needsSetup: a.backfillState === "setup" };
      }
      const id = status.accounts.length + 1;
      const email = id === 1 ? DEMO.email : "work@example.com";
      const a = accountOf(id, email);
      a.backfillState = "setup";
      a.sync.history = { state: "setup", window: "90d", phase: "", done: 0, total: 0, rate: 0, etaSecs: null, error: null, bytesTotal: 0, bytesText: 0 };
      status.accounts.push(a);
      status.account ??= email;
      await new Promise((r) => setTimeout(r, 900));
      return { accountId: id, email, messagesTotal: a.messagesTotal, threadsTotal: 31_020, needsSetup: true };
    }
    case "account_estimates":
      return [
        { window: "30d", messages: 1_240, bytes: 1_240 * 48 * 1024 },
        { window: "90d", messages: 4_210, bytes: 4_210 * 48 * 1024 },
        { window: "1y", messages: 15_800, bytes: 15_800 * 3 * 1024 + 4_210 * 45 * 1024 },
        { window: "all", messages: 48_312, bytes: 48_312 * 3 * 1024 + 4_210 * 45 * 1024 },
      ];
    case "finish_onboarding":
    case "resume_history":
    case "pause_history":
    case "set_sync_window": {
      const a = status.accounts.find((x) => x.id === args?.accountId);
      if (a) {
        if (args?.window) a.syncWindow = args.window as Account["syncWindow"];
        if (args?.notify) a.notify = args.notify as Account["notify"];
        const state = cmd === "pause_history" ? "paused" : "running";
        a.backfillState = state;
        a.sync.history = { state, window: a.syncWindow, phase: "index", done: 1_830, total: 15_800, rate: 46, etaSecs: 304, error: null, bytesTotal: 1.9e9, bytesText: 310e6 };
      }
      return;
    }
    case "set_account_settings": {
      const a = status.accounts.find((x) => x.id === args?.accountId);
      if (a) Object.assign(a, Object.fromEntries(Object.entries((args?.settings ?? {}) as object).filter(([, v]) => v !== undefined)));
      return;
    }
    case "stage_attachment":
      return { id: `s${Math.random().toString(36).slice(2, 8)}`, name: args?.name, mime: args?.mime || "application/octet-stream", size: args?.size };
    case "unstage_attachment":
      return;
    case "search_contacts": {
      const q = String(args?.query ?? "").toLowerCase();
      return seeds
        .filter((s) => s.fromName.toLowerCase().includes(q) || s.fromEmail.toLowerCase().includes(q))
        .map((s, i) => ({ name: s.fromName, email: s.fromEmail, frequency: 10 - i }));
    }
    case "get_attachment_data": {
      const part = Number(args?.part ?? 0);
      const filename = part === 0 ? "Q4-Planning.xlsx" : part === 1 ? "Architecture-Note.pdf" : "System-Diagram.png";
      const mime = part === 0 ? "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" : part === 1 ? "application/pdf" : "image/png";

      if (filename.endsWith(".xlsx")) {
        const XLSX = await import("xlsx");
        const wb = XLSX.utils.book_new();
        const ws1 = XLSX.utils.aoa_to_sheet([
          ["Task ID", "Feature / Deliverable", "Owner", "Story Points", "Priority", "Status"],
          ["DEMO-101", "IMAP IDLE push", "Mira Okonkwo", 8, "P0", "Shipped"],
          ["DEMO-102", "In-app spreadsheet preview", "Jules Park", 5, "P0", "Shipped"],
          ["DEMO-103", "Undo send (5–10s)", "Sam Ellison", 3, "P1", "Shipped"],
          ["DEMO-104", "Contact autocomplete", "Alex", 5, "P1", "Shipped"],
          ["DEMO-105", "Resizable split pane", "Jules Park", 3, "P2", "Shipped"],
        ]);
        const ws2 = XLSX.utils.aoa_to_sheet([
          ["Benchmark Metric", "Target Budget", "Measured", "Variance", "Outcome"],
          ["Cold Start Time", "< 1500 ms", "820 ms", "-45%", "Passed"],
          ["New Mail Notification", "< 2.0 s", "1.1 s", "-45%", "Passed"],
          ["SQLite FTS5 Query (10k msgs)", "< 15 ms", "4.2 ms", "-72%", "Passed"],
          ["Memory Footprint (Idle)", "< 60 MB", "42.5 MB", "-29%", "Passed"],
        ]);
        XLSX.utils.book_append_sheet(wb, ws1, "Sprint Deliverables");
        XLSX.utils.book_append_sheet(wb, ws2, "Latency Benchmarks");
        const b64 = XLSX.write(wb, { type: "base64", bookType: "xlsx" });
        return { filename, mime, size: 24576, dataBase64: b64 };
      }
      if (filename.endsWith(".png")) {
        const png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
        return { filename, mime, size: 108420, dataBase64: png };
      }
      return { filename, mime, size: 482133, dataBase64: btoa("%PDF-1.4 Mock PDF Document") };
    }
    case "account_storage":
      return { messages: 4210, dbBytes: 180e6, filesBytes: 240e6 };
    case "sign_out":
      return;
    case "set_autostart":
      status.autostart = Boolean(args?.enabled);
      return status.autostart;
    default:
      return;
  }
}
