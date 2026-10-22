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
  /** count → index → bodies → files; "" = not started. */
  phase: HistoryPhase;
  done: number;
  total: number;
  /** Smoothed messages per second (0 = unknown yet). */
  rate: number;
  etaSecs: number | null;
  /** Set when the download stopped after a failure (it retries itself). */
  error: string | null;
  /** Size of the range's messages known so far (bytes). */
  bytesTotal: number;
  /** Same without attachments (approximate). */
  bytesText: number;
}

export type HistoryPhase = "count" | "index" | "bodies" | "files" | "";
export type AttachMode = "none" | "limit" | "all";

export interface AccountSettings {
  displayName?: string;
  color?: string;
  signature?: string;
  categoryTabs?: boolean;
  attachMode?: AttachMode;
  attachMaxMb?: number;
}

export interface AccountStorage {
  messages: number;
  dbBytes: number;
  filesBytes: number;
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
  backfillPhase: HistoryPhase;
  backfillError: string | null;
  attachMode: AttachMode;
  attachMaxMb: number;
  displayName: string;
  /** Cached raster data URL; the webview never fetches Google photos. */
  avatarData: string | null;
  color: string;
  signature: string;
  /** Show Gmail's inbox tabs. */
  categoryTabs: boolean;
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

export type NotificationMode = "all" | "primary" | "otp" | "off";

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
  /** Gmail marked a message of the thread IMPORTANT. */
  important: boolean;
}

/** Gmail inbox tabs (system CATEGORY_* labels). */
export type Category = "CATEGORY_PERSONAL" | "CATEGORY_PROMOTIONS" | "CATEGORY_SOCIAL" | "CATEGORY_UPDATES" | "CATEGORY_FORUMS";

export interface CategoryCount {
  id: Category;
  unread: number;
  total: number;
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
  /** One-click sign-in / verification link (when no code). */
  otpLink?: string | null;
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

export type ComposeMode = "new" | "reply" | "reply_all" | "forward";

/** Prefill from Rust for a reply / reply all / forward. */
export interface ComposeInit {
  to: string;
  cc: string;
  subject: string;
  inReplyTo: string;
  references: string;
  threadId: string | null;
  originalFrom: string;
  originalTo: string;
  originalCc: string;
  originalDate: number;
  originalSubject: string;
  originalText: string;
  /** Forward: the original's files. */
  attachments: DraftAttachment[];
}

/** A file in compose: uploaded from this PC (staged) or from the forwarded mail. */
export interface DraftAttachment {
  name: string;
  mime: string;
  size: number;
  staged?: string | null;
  sourceMessage?: string | null;
  sourcePart?: number | null;
}

export interface StagedRef {
  id: string;
  name: string;
  mime: string;
  size: number;
}

export interface Quote {
  header: string;
  text: string;
  forward: boolean;
}

/** What a compose window holds; sent as-is to save_draft / send_message. */
export interface Draft {
  to: string;
  cc: string;
  bcc: string;
  subject: string;
  body: string;
  quote: Quote | null;
  inReplyTo: string;
  references: string;
  threadId: string | null;
  draftId: string | null;
  /** Rich body from the editor (sanitized again in Rust); null = plain text. */
  html: string | null;
  attachments: DraftAttachment[];
}

export interface Sending {
  sendId: number;
  draftId: string;
  delaySecs: number;
}

export interface SendResult {
  sendId: number;
  status: "sent" | "failed";
  message: string | null;
}
