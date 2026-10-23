// Mock backend for browser-only UI work (`pnpm dev`). Never used in Tauri.
import type { Account, Activity, AppStatus, Label, Message, RenderedMessage, SyncStatus, ThreadRow } from "./types";
import { threadKey, type SelectionSource, type ThreadKey } from "./selection";
import { needsServer } from "./search";

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
  ["Elena Vance", "elena@acme-systems.io", "Sprint roadmap & deliverables", "Hi Abdallah, attached is the updated sprint allocation and release roadmap for Q4…", null],
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
const catOf = (t: ThreadRow) => (t.otpCode || t.threadId === "t2" ? "CATEGORY_PERSONAL" : CATS[Number(t.threadId.slice(1)) % CATS.length]);

function message(t: ThreadRow): Message {
  const html = t.otpCode
    ? `<div style="font-family:Segoe UI,Arial,sans-serif;max-width:520px;margin:24px auto;padding:24px;border:1px solid #333;border-radius:12px;background:#18181b;color:#f4f4f5"><h2 style="margin:0 0 12px;color:#fff">Verify your sign-in</h2><p style="color:#a1a1aa">Use this verification code to securely access your account:</p><p style="font-size:36px;letter-spacing:8px;font-weight:700;color:#38bdf8;margin:20px 0;font-family:monospace">${t.otpCode}</p><p style="color:#71717a;font-size:12px">If you didn't request this code, you can safely disregard this email. <a href="https://example.com/help" style="color:#38bdf8">Help Center</a></p><img src="https://tracker.example.com/pixel.gif" width="1" height="1"></div>`
    : t.threadId === "t2"
    ? `<div style="font-family:Segoe UI,-apple-system,BlinkMacSystemFont,Roboto,sans-serif;max-width:680px;line-height:1.6;color:#e4e4e7">
        <p style="margin-top:0">Hi Abdallah,</p>
        <p>Attached is the updated sprint planning allocation and release roadmap for Q4. We've mapped out the key milestones, performance budgets, and core team deliverables.</p>
        <div style="background:#18181b;border:1px solid #27272a;border-radius:12px;padding:18px;margin:20px 0">
          <h4 style="margin:0 0 10px;color:#fafafa;font-size:14px;display:flex;align-items:center;gap:8px">📋 Deliverables & Attachments Overview</h4>
          <ul style="margin:0;padding-left:20px;color:#a1a1aa;font-size:13px;line-height:1.7">
            <li><strong style="color:#f4f4f5">Sprint-Roadmap-Q4.xlsx:</strong> Task milestones, sprint story points, and latency benchmarks.</li>
            <li><strong style="color:#f4f4f5">Architecture-Spec.pdf:</strong> IPC pipeline, SQLite indexing, and zero-disk streaming specs.</li>
            <li><strong style="color:#f4f4f5">System-Diagram.png:</strong> Core architecture flow: SQLite cache, IMAP push, and Tauri v2.</li>
          </ul>
        </div>
        <p>Please review the attached spreadsheet using the in-app previewer and let me know if you need any adjustments before the planning sync.</p>
        <p style="margin-top:28px">Best regards,<br><strong style="color:#fff">Elena Vance</strong><br><span style="color:#71717a;font-size:12px">Staff Engineer & Tech Lead · Acme Systems</span></p>
      </div>`
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
    attachments: t.threadId === "t2" ? [
      { part: 0, filename: "Sprint-Roadmap-Q4.xlsx", mime: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", size: 24_576, inline: false },
      { part: 1, filename: "Architecture-Spec.pdf", mime: "application/pdf", size: 482_133, inline: false },
      { part: 2, filename: "System-Diagram.png", mime: "image/png", size: 108_420, inline: false },
    ] : [],
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
    avatarData: null,
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
  { at: now - 60_000, account: "me@gmail.com", level: "success", text: "Inbox ready: 50 newest messages in 1.0 s" },
  { at: now - 30_000, account: "me@gmail.com", level: "info", text: "Live: Gmail will push new mail instantly" },
];

let selectionId = 0;
let selectedKeys: ThreadKey[] = [];
let excludedKeys = new Set<string>();

function mockSearch(query: string) {
  const tokens = query.match(/(?:[^\s"]+|"[^"]*")+/g) ?? [];
  return threads.filter((t) => tokens.every((raw) => {
    const token = raw.toLowerCase();
    if (token === "is:unread") return t.unread;
    if (token === "is:read") return !t.unread;
    if (token === "is:starred") return t.starred;
    if (token === "has:attachment") return false;
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
          (source.labelId === "INBOX" || source.labelId === "STARRED" && t.starred) && (!source.category || catOf(t) === source.category)));
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
      return labels;
    case "list_threads": {
      const matches = args?.labelId === "INBOX"
        ? threads.filter((t) => !args?.category || catOf(t) === args.category)
        : threads.filter((t) => (args?.labelId === "STARRED" ? t.starred : false));
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
      const reconnectId = args?.accountId;
      if (reconnectId != null) {
        const a = status.accounts.find((a) => a.id === reconnectId);
        if (!a) throw new Error("Account not found.");
        return { accountId: a.id, email: a.email, messagesTotal: a.messagesTotal, threadsTotal: 31_020, needsSetup: a.backfillState === "setup" };
      }
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
    case "search_contacts": {
      const q = String(args?.query ?? "").toLowerCase();
      return people
        .filter(([name, email]) => name.toLowerCase().includes(q) || email.toLowerCase().includes(q))
        .map(([name, email], i) => ({ name, email, frequency: 10 - i }));
    }
    case "save_attachment":
      return { path: `C:\\Downloads\\${args?.name || "file"}`, opened: false, blocked: false };
    case "get_attachment_data": {
      const part = Number(args?.part ?? 0);
      const filename = part === 0 ? "Sprint-Roadmap-Q4.xlsx" : part === 1 ? "Architecture-Spec.pdf" : "System-Diagram.png";
      const mime = part === 0 ? "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" : part === 1 ? "application/pdf" : "image/png";
      
      if (filename.endsWith(".xlsx")) {
        const XLSX = await import("xlsx");
        const wb = XLSX.utils.book_new();
        const ws1 = XLSX.utils.aoa_to_sheet([
          ["Task ID", "Feature / Deliverable", "Owner", "Story Points", "Priority", "Status"],
          ["PULSE-101", "IMAP IDLE Push Engine", "Alex Chen", 8, "P0", "Shipped"],
          ["PULSE-102", "In-App Spreadsheet & Doc Preview", "Elena Vance", 5, "P0", "Shipped"],
          ["PULSE-103", "Undo Send Grace Queue (5-10s)", "Marcus Brody", 3, "P1", "Shipped"],
          ["PULSE-104", "Contact Frequency Autocomplete", "Sarah Jenkins", 5, "P1", "Shipped"],
          ["PULSE-105", "Resizable Split Pane Divider", "Elena Vance", 3, "P2", "Shipped"],
          ["PULSE-106", "Cross-Platform Packaging (Win/Mac/Linux)", "DevOps Team", 5, "P1", "Verified"]
        ]);
        const ws2 = XLSX.utils.aoa_to_sheet([
          ["Benchmark Metric", "Target Budget", "Measured (v0.2.0)", "Variance", "Outcome"],
          ["Cold Start Time", "< 1500 ms", "820 ms", "-45%", "Passed"],
          ["New Mail Notification", "< 2.0 s", "1.1 s", "-45%", "Passed"],
          ["SQLite FTS5 Query (10k msgs)", "< 15 ms", "4.2 ms", "-72%", "Passed"],
          ["Memory Footprint (Idle)", "< 60 MB", "42.5 MB", "-29%", "Passed"],
          ["Attachment Decryption & Stream", "< 100 ms", "28 ms", "-72%", "Passed"]
        ]);
        XLSX.utils.book_append_sheet(wb, ws1, "Sprint Deliverables");
        XLSX.utils.book_append_sheet(wb, ws2, "Latency Benchmarks");
        const b64 = XLSX.write(wb, { type: "base64", bookType: "xlsx" });
        return { filename, mime, size: 24576, dataBase64: b64 };
      }
      if (filename.endsWith(".png")) {
        // Simple 1x1 png
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
