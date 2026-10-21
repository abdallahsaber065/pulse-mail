// Thin typed wrapper over Tauri IPC. Outside Tauri (plain `pnpm dev` in a
// browser) it falls back to mock data so the UI can be designed quickly.
import type {
  Activity,
  AppStatus,
  Label,
  Message,
  NotificationMode,
  OpenTarget,
  PrefKey,
  Prefs,
  RenderedMessage,
  SavedAttachment,
  SignedIn,
  SyncStatus,
  SyncWindow,
  ThreadRow,
  WindowEstimate,
} from "./types";
import * as mock from "./mock";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri) return mock.handle(cmd, args) as Promise<T>;
  const { invoke } = await import("@tauri-apps/api/core");
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    // Commands reject with { message } holding a friendly sentence.
    const message = (e as { message?: string })?.message ?? "Something went wrong. Please try again.";
    throw new Error(message);
  }
}

/** `acct`: an account id, or null = every account (unified views) / default. */
type Acct = number | null;

export const api = {
  status: () => call<AppStatus>("get_status"),
  signIn: () => call<SignedIn>("sign_in"),
  signOut: (accountId: Acct = null) => call<void>("sign_out", { accountId }),
  estimates: (accountId: number) => call<WindowEstimate[]>("account_estimates", { accountId }),
  finishOnboarding: (accountId: number, window: SyncWindow, notify: NotificationMode) =>
    call<void>("finish_onboarding", { accountId, window, notify }),
  setSyncWindow: (accountId: number, window: SyncWindow) => call<void>("set_sync_window", { accountId, window }),
  pauseHistory: (accountId: number) => call<void>("pause_history", { accountId }),
  resumeHistory: (accountId: number) => call<void>("resume_history", { accountId }),
  labels: (accountId: Acct = null) => call<Label[]>("list_labels", { accountId }),
  threads: (labelId: string, limit = 200, offset = 0, accountId: Acct = null) =>
    call<ThreadRow[]>("list_threads", { labelId, limit, offset, accountId }),
  search: (query: string, limit = 100, accountId: Acct = null) => call<ThreadRow[]>("search", { query, limit, accountId }),
  thread: (threadId: string, accountId: Acct = null) => call<Message[]>("get_thread", { threadId, accountId }),
  /** Like `thread`, but first downloads bodies that are not cached yet. */
  loadThread: (threadId: string, accountId: Acct = null) => call<Message[]>("load_thread", { threadId, accountId }),
  modifyThread: (threadId: string, add: string[], remove: string[], accountId: Acct = null) =>
    call<void>("modify_thread", { threadId, add, remove, accountId }),
  activity: () => call<Activity[]>("get_activity"),
  setNotifications: (mode: NotificationMode, accountId: Acct = null) => call<void>("set_notifications", { mode, accountId }),
  /** `manual` = the user asked; automatic wake-ups pass false and stay quiet. */
  syncNow: (manual = true, accountId: Acct = null) => call<void>("sync_now", { manual, accountId }),
  takePendingOpen: () => call<OpenTarget | null>("take_pending_open"),
  /** `secret`: a verification code; cleared from the clipboard later (pref). */
  copyText: (text: string, secret = false) => call<void>("copy_text", { text, secret }),
  renderMessage: (messageId: string, dark: boolean, showImages: boolean, accountId: Acct = null) =>
    call<RenderedMessage>("render_message", { messageId, dark, showImages, accountId }),
  setImageSender: (email: string, allowed: boolean, accountId: Acct = null) =>
    call<void>("set_image_sender", { email, allowed, accountId }),
  setPref: <K extends PrefKey>(key: K, value: Prefs[K]) => call<void>("set_pref", { key, value }),
  saveAttachment: (messageId: string, part: number, open: boolean, accountId: Acct = null) =>
    call<SavedAttachment>("save_attachment", { messageId, part, open, accountId }),
  openExternal: (url: string) => call<void>("open_external", { url }),
  setDestroyOnClose: (enabled: boolean) => call<void>("set_destroy_on_close", { enabled }),
  setAutostart: (enabled: boolean) => call<boolean>("set_autostart", { enabled }),
};

type Unlisten = () => void;

export async function onEvent<T>(name: "mail-updated" | "sync-status" | "sync-activity" | "open-thread", cb: (payload: T) => void): Promise<Unlisten> {
  if (!inTauri) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<T>(name, (e) => cb(e.payload));
}

export type { SyncStatus };
