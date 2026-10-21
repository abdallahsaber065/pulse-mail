// Mock backend for browser-only UI work (`pnpm dev`). Never used in Tauri.
import type { AppStatus, Label, Message, ThreadRow } from "./types";

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
}));

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
    hasAttachments: false,
    otpCode: t.otpCode,
  };
}

const status: AppStatus = {
  clientConfigured: true,
  clientConfigPath: "%APPDATA%\\com.abdallah.pulsemail\\google-oauth-client.json",
  account: "me@gmail.com",
  sync: { running: true, imapConnected: true, syncing: false, lastSyncAt: now, error: null },
  destroyOnClose: false,
  autostart: true,
};

export async function handle(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  switch (cmd) {
    case "get_status":
      return structuredClone(status);
    case "list_labels":
      return labels;
    case "list_threads":
      return args?.labelId === "INBOX" ? threads : threads.filter((t) => (args?.labelId === "STARRED" ? t.starred : false));
    case "search": {
      const q = String(args?.query ?? "").toLowerCase();
      return threads.filter((t) => `${t.subject} ${t.fromName} ${t.snippet}`.toLowerCase().includes(q));
    }
    case "get_thread": {
      const t = threads.find((x) => x.threadId === args?.threadId);
      return t ? [message(t)] : [];
    }
    case "take_pending_open":
      return null;
    case "copy_text":
      await navigator.clipboard?.writeText(String(args?.text ?? ""));
      return;
    case "open_external":
      window.open(String(args?.url), "_blank", "noopener");
      return;
    case "set_autostart":
      return Boolean(args?.enabled);
    default:
      return;
  }
}
