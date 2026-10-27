// Thin typed wrapper over Tauri IPC. Outside Tauri (plain `pnpm dev` in a
// browser) it falls back to the demo mailbox. A Tauri *dev* build can opt
// in with `PULSE_MAIL_DEMO=1`; release builds never take that path.
import type {
  Category,
  CategoryCount,
  AccountSettings,
  AccountStorage,
  Activity,
  AttachmentData,
  ComposeInit,
  ComposeMode,
  UpdateAvail,
  ContactSuggestion,
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
import type { SelectionInfo, SelectionSource, ThreadKey, ReadBatch } from "./selection";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Browser `pnpm dev`, or `PULSE_MAIL_DEMO=1` during `pnpm tauri dev`. Always false in release. */
export const isDemo =
  !inTauri || (import.meta.env.DEV === true && import.meta.env.VITE_PULSE_MAIL_DEMO === "1");

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isDemo) return mock.handle(cmd, args) as Promise<T>;
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
  signIn: (accountId: Acct = null) => call<SignedIn>("sign_in", { accountId }),
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
    if (isDemo) return mock.handle("stage_attachment", { name: file.name, mime: file.type, size: file.size }) as Promise<StagedRef>;
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
  search: (query: string, limit = 100, accountId: Acct = null, offset = 0) => call<ThreadRow[]>("search", { query, limit, accountId, offset }),
  composeContext: (messageId: string, mode: Exclude<ComposeMode, "new">, accountId: Acct = null) =>
    call<ComposeInit>("compose_context", { messageId, mode, accountId }),
  saveDraft: (draft: Draft, accountId: Acct = null) =>
    call<{ draftId: string; threadId: string | null }>("save_draft", { draft, accountId }),
  deleteDraft: (draftId: string, accountId: Acct = null) => call<void>("delete_draft", { draftId, accountId }),
  sendMessage: (draft: Draft, accountId: Acct = null) => call<Sending>("send_message", { draft, accountId }),
  cancelSend: (sendId: number) => call<boolean>("cancel_send", { sendId }),
  searchContacts: (query: string, accountId: Acct = null) =>
    call<ContactSuggestion[]>("search_contacts", { query, accountId }),
  /** Gmail server search (all mail, every operator); rows are cached locally. */
  searchServer: (query: string, limit = 50, accountId: Acct = null) =>
    call<ThreadRow[]>("search_server", { query, limit, accountId }),
  thread: (threadId: string, accountId: Acct = null) => call<Message[]>("get_thread", { threadId, accountId }),
  /** Like `thread`, but first downloads bodies that are not cached yet. */
  loadThread: (threadId: string, accountId: Acct = null) => call<Message[]>("load_thread", { threadId, accountId }),
  modifyThread: (threadId: string, add: string[], remove: string[], accountId: Acct = null) =>
    call<void>("modify_thread", { threadId, add, remove, accountId }),
  createSelection: (source: SelectionSource) => call<SelectionInfo>("create_selection", { source }),
  selectionMembers: (selectionId: number, threads: ThreadKey[]) => call<ThreadKey[]>("selection_members", { selectionId, threads }),
  markSelectionRead: (selectionId: number, offset: number, excluded: ThreadKey[]) =>
    call<ReadBatch>("mark_selection_read", { selectionId, offset, excluded }),
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
  getAttachmentData: (messageId: string, part: number, accountId: Acct = null) =>
    call<AttachmentData>("get_attachment_data", { messageId, part, accountId }),
  openExternal: (url: string) => call<void>("open_external", { url }),
  /** Reveal the local logs folder in Explorer (Settings → About). */
  openLogsDir: () => call<void>("open_logs_dir"),
  /** Check GitHub Releases for a newer version (null = up to date). */
  updateCheck: () => call<UpdateAvail | null>("update_check"),
  /** Install the pending update: background bytes when present, else download + install. */
  updateInstall: () => call<void>("update_install"),
  setDestroyOnClose: (enabled: boolean) => call<void>("set_destroy_on_close", { enabled }),
  setAutostart: (enabled: boolean) => call<boolean>("set_autostart", { enabled }),
};

type Unlisten = () => void;

export async function onEvent<T>(name: "mail-updated" | "sync-status" | "sync-activity" | "open-thread" | "send-result" | "accounts-changed" | "update-available" | "update-downloading" | "update-ready", cb: (payload: T) => void): Promise<Unlisten> {
  if (isDemo) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<T>(name, (e) => cb(e.payload));
}

export type { SyncStatus };
