// Mirrors the serde types in src-tauri/src (camelCase).

export interface Progress {
  label: string;
  done: number;
  total: number;
}

export interface SyncStatus {
  running: boolean;
  imapConnected: boolean;
  syncing: boolean;
  lastSyncAt: number | null;
  error: string | null;
  /** Foreground work the user is waiting on (first page, new mail). */
  progress: Progress | null;
  /** Background work (older mail, body prefetch). */
  backfill: Progress | null;
}

export interface Activity {
  at: number;
  level: "info" | "success" | "warn" | "error";
  text: string;
}

export type NotificationMode = "all" | "otp" | "off";

export interface AppStatus {
  clientConfigured: boolean;
  clientConfigPath: string;
  account: string | null;
  sync: SyncStatus;
  destroyOnClose: boolean;
  autostart: boolean;
  notifications: NotificationMode;
  dataDir: string;
  version: string;
}

export interface Label {
  id: string;
  name: string;
  kind: "system" | "user";
  unread: number;
  total: number;
}

export interface ThreadRow {
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
}

export interface OpenTarget {
  threadId: string;
  messageId: string | null;
}
