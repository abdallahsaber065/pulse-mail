// Mock backend for browser-only UI work (`pnpm dev`). Never used in Tauri.
import type { Account, Activity, AppStatus, Label, Message, RenderedMessage, SyncStatus, ThreadRow } from "./types";

const now = Date.now();
const min = 60_000;

const labels: Label[] = [
  { id: "INBOX", name: "INBOX", kind: "system", unread: 3, total: 9 },
  { id: "STARRED", name: "STARRED", kind: "system", unread: 0, total: 1 },
  { id: "SENT", name: "SENT", kind: "system", unread: 0, total: 0 },
  { id: "DRAFT", name: "DRAFT", kind: "system", unread: 0, total: 0 },
  { id: "Label_1", name: "Clients", kind: "user", unread: 1, total: 4 },
  { id: "Label_2", name: "Receipts", kind: "user", unread: 0, total: 2 },
];

const people = [
  ["GitHub", "noreply@github.com", "[GitHub] Please verify your device", "Verification code: 731590 If you did not attempt to sign in…", "731590"],
  ["Google", "no-reply@accounts.google.com", "Security alert", "A new sign-in on Windows. If this was you, you don't need to do anything.", null],
  ["Sara Ahmed", "sara@example.com", "Q4 report: final numbers", "Hi Abdallah, attached are the final numbers for the Q4 report. Can you take a look before…", null],
  ["Vercel", "notifications@vercel.com", "Deployment ready", "Your deployment for animeslayer is ready. Visit the preview at…", null],
  ["Instagram", "security@mail.instagram.com", "550912 is your Instagram code", "550912 is your Instagram code. Don't share it.", "550912"],
  ["Linear", "notifications@linear.app", "3 issues assigned to you", "SMR-112 Fix export, SMR-118 Gate 3 review, SMR-121 Docs cleanup", null],
  ["Amazon.eg", "order-update@amazon.eg", "Your order has shipped", "Order #402-1188 has shipped. Arriving Thursday.", null],
  ["Mohamed", "mohamed@example.com", "Weekend?", "Are we still on for Friday? Let me know and I'll book the place.", null],
  ["The Rust Blog", "blog@rust-lang.org", "Announcing Rust 1.99.1", "The Rust team has published a new point release of Rust, 1.99.1…", null],
] as const;

const threads: ThreadRow[] = people.map(([fromName, fromEmail, subject, snippet, otp], i) => ({
  accountId: 1,
  threadId: `t${i}`,
  lastMessageId: `m${i}`,
  subject,
  fromName,
  fromEmail,
  snippet,
  internalDate: now - i * i * 37 * min,
  messageCount: i === 2 ? 3 : 1,
  unread: i < 3,
  starred: i === 2,
  otpCode: otp,
  important: i % 3 === 0,
}));
const CATS = ["CATEGORY_PERSONAL", "CATEGORY_UPDATES", "CATEGORY_PROMOTIONS", "CATEGORY_SOCIAL"];
const catOf = (t: ThreadRow) => (t.otpCode ? "CATEGORY_PERSONAL" : CATS[Number(t.threadId.slice(1)) % CATS.length]);

function message(t: ThreadRow): Message {
  const html = t.otpCode
    ? `<div style="font-family:Segoe UI,Arial;max-width:520px;margin:24px auto;padding:24px;border:1px solid #ddd;border-radius:12px"><h2 style="margin:0 0 12px">Verify it's you</h2><p>Use this code to finish signing in:</p><p style="font-size:32px;letter-spacing:6px;font-weight:700">${t.otpCode}</p><p style="color:#666">If you didn't request this, you can ignore this email. <a href="https://example.com/help">Get help</a></p><img src="https://tracker.example.com/pixel.gif" width="1" height="1"></div>`
    : null;
  return {
    id: t.lastMessageId,
    threadId: t.threadId,
    internalDate: t.internalDate,
    fromName: t.fromName,
    fromEmail: t.fromEmail,
    toList: "me@gmail.com",
    subject: t.subject,
    snippet: t.snippet,
    bodyText: `${t.snippet}\n\nThanks,\n${t.fromName}`,
    bodyHtml: html,
    labelIds: ["INBOX"],
    unread: t.unread,
    starred: t.starred,
    hasAttachments: t.threadId === "t2",
    otpCode: t.otpCode,
    bodyLoaded: true,
    ccList: "",
    replyTo: "",
    rfcMessageId: `<${t.lastMessageId}@mock>`,
    rfcReferences: "",
    attachments: t.threadId === "t2" ? [{ part: 0, filename: "Q4-report.pdf", mime: "application/pdf", size: 482_133, inline: false }] : [],
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
    displayName: "",
    color: "",
    signature: "",
    categoryTabs: true,
    sync: syncOf(id, email),
    unread: 3,
  };
}

// `?onboarding` in the URL starts with no account (first-run flow).
const firstRun = typeof location !== "undefined" && location.search.includes("onboarding");

const status: AppStatus = {
  clientConfigured: true,
  clientConfigPath: "%APPDATA%\\com.abdallah.pulsemail\\google-oauth-client.json",
  account: firstRun ? null : "me@gmail.com",
  accounts: firstRun ? [] : [accountOf(1, "me@gmail.com")],
  dbBytes: 182_000_000,
  destroyOnClose: false,
  autostart: true,
  notifications: "all",
  dataDir: "%APPDATA%\\com.abdallah.pulsemail",
  version: "0.1.0",
  prefs: {
    images: "ask",
    email_dark: "smart",
    otp_autocopy: "0",
    clipboard_clear_secs: "60",
    otp_lockscreen: "hide",
    undo_send_secs: "5",
    storage_budget_mb: "1024",
  },
};

function mockRender(m: Message, dark: boolean): RenderedMessage {
  const esc = (s: string) => s.replace(/[&<>"]/g, (c) => `&#${c.charCodeAt(0)};`);
  const color = dark ? "#e3e3e3" : "#1f1f1f";
  const body = m.bodyHtml ?? `<div style="white-space:pre-wrap">${esc(m.bodyText)}</div>`;
  return {
    srcdoc: `<!doctype html><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:"><body dir="auto" style="margin:0;font:14px/1.5 Segoe UI,sans-serif;color:${color}">${body}</body>`,
    remoteImages: m.bodyHtml ? 1 : 0,
    imagesShown: 0,
    trackers: m.bodyHtml ? 1 : 0,
    plainText: !m.bodyHtml,
    sender: m.fromEmail,
    senderAllowed: false,
  };
}

const activity: Activity[] = [
  { at: now - 60_000, account: "me@gmail.com", level: "success", text: "Inbox ready: 50 newest messages in 1.0 s" },
  { at: now - 30_000, account: "me@gmail.com", level: "info", text: "Live: Gmail will push new mail instantly" },
];

export async function handle(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  switch (cmd) {
    case "get_status":
      return structuredClone(status);
    case "list_labels":
      return labels;
    case "list_threads":
      return args?.labelId === "INBOX"
        ? threads.filter((t) => !args?.category || catOf(t) === args.category)
        : threads.filter((t) => (args?.labelId === "STARRED" ? t.starred : false));
    case "category_counts":
      return ["CATEGORY_PERSONAL", "CATEGORY_PROMOTIONS", "CATEGORY_SOCIAL", "CATEGORY_UPDATES", "CATEGORY_FORUMS"].map((id) => {
        const ts = threads.filter((t) => catOf(t) === id);
        return { id, unread: ts.filter((t) => t.unread).length, total: ts.length };
      });
    case "otp_feedback":
      if (!args?.useful) for (const t of threads) if (t.lastMessageId === args?.messageId) t.otpCode = null;
      return null;
    case "search": {
      const q = String(args?.query ?? "").toLowerCase();
      return threads.filter((t) => `${t.subject} ${t.fromName} ${t.snippet}`.toLowerCase().includes(q));
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
        originalTo: "me@gmail.com",
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
        if (remove.includes("INBOX")) threads.splice(threads.indexOf(t), 1);
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
      return { path: "C:\\Users\\me\\Downloads\\Q4-report.pdf", opened: Boolean(args?.open), blocked: false };
    case "take_pending_open":
      return null;
    case "copy_text":
      await navigator.clipboard?.writeText(String(args?.text ?? ""));
      return;
    case "open_external":
      window.open(String(args?.url), "_blank", "noopener");
      return;
    case "sign_in": {
      const id = status.accounts.length + 1;
      const email = id === 1 ? "me@gmail.com" : "work@example.com";
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
