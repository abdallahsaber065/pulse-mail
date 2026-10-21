// Mirrors the serde types in src-tauri/src (camelCase).

export interface Progress {
  label: string;
  done: number;
  total: number;
}

export interface SyncStatus {
  accountId: number;
  email: string;
  running: boolean;
  imapConnected: boolean;
  syncing: boolean;
  /** The current pass is user-initiated (or the first sync); otherwise quiet. */
  foreground: boolean;
  lastSyncAt: number | null;
  error: string | null;
  /** Foreground work the user is waiting on (first page, new mail). */
  progress: Progress | null;
  /** Background work (older mail, body prefetch). */
  backfill: Progress | null;
  /** History download (H1) state. */
  history: History | null;
}

export type SyncWindow = "30d" | "90d" | "1y" | "all";
export type HistoryState = "setup" | "pending" | "running" | "paused" | "done";

export interface History {
  state: HistoryState;
  window: SyncWindow;
  /** "list" = message list, "bodies" = offline copies, "" = idle. */
  phase: "list" | "bodies" | "";
  done: number;
  total: number;
  /** Messages per second this session (0 = unknown yet). */
  rate: number;
  etaSecs: number | null;
}

export interface Account {
  id: number;
  email: string;
  historyId: string | null;
  lastSyncAt: number | null;
  syncWindow: SyncWindow;
  notify: NotificationMode;
  backfillState: HistoryState;
  backfillDone: number;
  backfillTotal: number;
  messagesTotal: number;
  sync: SyncStatus;
  unread: number;
}

export interface SignedIn {
  accountId: number;
  email: string;
  messagesTotal: number;
  threadsTotal: number;
  needsSetup: boolean;
}

export interface WindowEstimate {
  window: SyncWindow;
  messages: number;
  bytes: number;
}

export interface Activity {
  at: number;
  /** Account email ("" before any account). */
  account: string;
  level: "info" | "success" | "warn" | "error";
  text: string;
}

export type NotificationMode = "all" | "otp" | "off";

export interface AppStatus {
  clientConfigured: boolean;
  clientConfigPath: string;
  /** Default (oldest) account email. */
  account: string | null;
  accounts: Account[];
  destroyOnClose: boolean;
  autostart: boolean;
  notifications: NotificationMode;
  dataDir: string;
  version: string;
  /** Backend prefs (src-tauri/src/prefs.rs). */
  prefs: Prefs;
  /** Database size in bytes. */
  dbBytes: number;
}

export interface Prefs {
  images: "ask" | "always";
  email_dark: "smart" | "original";
  otp_autocopy: "0" | "1";
  clipboard_clear_secs: "0" | "30" | "60" | "120";
  otp_lockscreen: "hide" | "show";
  undo_send_secs: "0" | "5" | "10" | "20" | "30";
  offline_days: "30" | "90" | "365";
  storage_budget_mb: "512" | "1024" | "2048" | "5120";
}

export type PrefKey = keyof Prefs;

export interface Attachment {
  part: number;
  filename: string;
  mime: string;
  size: number;
  inline: boolean;
}

export interface RenderedMessage {
  srcdoc: string;
  remoteImages: number;
  imagesShown: number;
  trackers: number;
  plainText: boolean;
  sender: string;
  senderAllowed: boolean;
}

export interface SavedAttachment {
  path: string;
  opened: boolean;
  blocked: boolean;
}

export interface Label {
  id: string;
  name: string;
  kind: "system" | "user";
  unread: number;
  total: number;
}

export interface ThreadRow {
  accountId: number;
  threadId: string;
  lastMessageId: string;
  subject: string;
  fromName: string;
  fromEmail: string;
  snippet: string;
  internalDate: number;
  messageCount: number;
  unread: boolean;
  starred: boolean;
  otpCode: string | null;
}

export interface Message {
  id: string;
  threadId: string;
  internalDate: number;
  fromName: string;
  fromEmail: string;
  toList: string;
  subject: string;
  snippet: string;
  bodyText: string;
  bodyHtml: string | null;
  labelIds: string[];
  unread: boolean;
  starred: boolean;
  hasAttachments: boolean;
  otpCode: string | null;
  /** false = only headers + snippet are cached; the full body loads on open. */
  bodyLoaded: boolean;
  ccList: string;
  replyTo: string;
  rfcMessageId: string;
  rfcReferences: string;
  attachments: Attachment[];
}

export interface OpenTarget {
  accountId: number;
  threadId: string;
  messageId: string | null;
}
