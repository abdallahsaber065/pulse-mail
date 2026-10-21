// Mirrors the serde types in src-tauri/src (camelCase).

export interface SyncStatus {
  running: boolean;
  imapConnected: boolean;
  syncing: boolean;
  lastSyncAt: number | null;
  error: string | null;
}

export interface AppStatus {
  clientConfigured: boolean;
  clientConfigPath: string;
  account: string | null;
  sync: SyncStatus;
  destroyOnClose: boolean;
  autostart: boolean;
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
}

export interface OpenTarget {
  threadId: string;
  messageId: string | null;
}
