// Thin typed wrapper over Tauri IPC. Outside Tauri (plain `pnpm dev` in a
// browser) it falls back to mock data so the UI can be designed quickly.
import type {
  Category,
  CategoryCount,
  AccountSettings,
  AccountStorage,
  Activity,
  ComposeInit,
  ComposeMode,
  Draft,
  Sending,
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
  StagedRef,
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
  setAccountSettings: (accountId: number, settings: AccountSettings) => call<void>("set_account_settings", { accountId, settings }),
  accountStorage: (accountId: number) => call<AccountStorage>("account_storage", { accountId }),
  /** Upload one file for compose as a raw IPC body (no base64). */
  stageAttachment: async (file: File): Promise<StagedRef> => {
    if (!inTauri) return mock.handle("stage_attachment", { name: file.name, mime: file.type, size: file.size }) as Promise<StagedRef>;
    const { invoke } = await import("@tauri-apps/api/core");
    const bytes = new Uint8Array(await file.arrayBuffer());
    try {
      return await invoke<StagedRef>("stage_attachment", bytes, {
        headers: { "x-name": encodeURIComponent(file.name), "x-mime": file.type || "application/octet-stream" },
      });
    } catch (e) {
      throw new Error((e as { message?: string })?.message ?? "Couldn't attach that file.");
    }
  },
  unstageAttachment: (id: string) => call<void>("unstage_attachment", { id }),
  labels: (accountId: Acct = null) => call<Label[]>("list_labels", { accountId }),
  threads: (labelId: string, limit = 200, offset = 0, accountId: Acct = null, category: Category | null = null) =>
    call<ThreadRow[]>("list_threads", { labelId, limit, offset, accountId, category }),
  categoryCounts: (accountId: Acct = null) => call<CategoryCount[]>("category_counts", { accountId }),
  /** "Not a code" (useful=false, clears it on that message) or a copied code. */
  otpFeedback: (accountId: number, sender: string, useful: boolean, messageId: string | null = null) =>
    call<void>("otp_feedback", { accountId, sender, useful, messageId }),
  search: (query: string, limit = 100, accountId: Acct = null) => call<ThreadRow[]>("search", { query, limit, accountId }),
  composeContext: (messageId: string, mode: Exclude<ComposeMode, "new">, accountId: Acct = null) =>
    call<ComposeInit>("compose_context", { messageId, mode, accountId }),
  saveDraft: (draft: Draft, accountId: Acct = null) =>
    call<{ draftId: string; threadId: string | null }>("save_draft", { draft, accountId }),
  deleteDraft: (draftId: string, accountId: Acct = null) => call<void>("delete_draft", { draftId, accountId }),
  sendMessage: (draft: Draft, accountId: Acct = null) => call<Sending>("send_message", { draft, accountId }),
  cancelSend: (sendId: number) => call<boolean>("cancel_send", { sendId }),
  /** Gmail server search (all mail, every operator); rows are cached locally. */
  searchServer: (query: string, limit = 50, accountId: Acct = null) =>
    call<ThreadRow[]>("search_server", { query, limit, accountId }),
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

export async function onEvent<T>(name: "mail-updated" | "sync-status" | "sync-activity" | "open-thread" | "send-result" | "accounts-changed", cb: (payload: T) => void): Promise<Unlisten> {
  if (!inTauri) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<T>(name, (e) => cb(e.payload));
}

export type { SyncStatus };
