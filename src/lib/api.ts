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
  SyncStatus,
  ThreadRow,
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

export const api = {
  status: () => call<AppStatus>("get_status"),
  signIn: () => call<string>("sign_in"),
  signOut: () => call<void>("sign_out"),
  labels: () => call<Label[]>("list_labels"),
  threads: (labelId: string, limit = 200, offset = 0) => call<ThreadRow[]>("list_threads", { labelId, limit, offset }),
  search: (query: string, limit = 100) => call<ThreadRow[]>("search", { query, limit }),
  thread: (threadId: string) => call<Message[]>("get_thread", { threadId }),
  /** Like `thread`, but first downloads bodies that are not cached yet. */
  loadThread: (threadId: string) => call<Message[]>("load_thread", { threadId }),
  modifyThread: (threadId: string, add: string[], remove: string[]) => call<void>("modify_thread", { threadId, add, remove }),
  activity: () => call<Activity[]>("get_activity"),
  setNotifications: (mode: NotificationMode) => call<void>("set_notifications", { mode }),
  syncNow: () => call<void>("sync_now"),
  takePendingOpen: () => call<OpenTarget | null>("take_pending_open"),
  /** `secret`: a verification code; cleared from the clipboard later (pref). */
  copyText: (text: string, secret = false) => call<void>("copy_text", { text, secret }),
  renderMessage: (messageId: string, dark: boolean, showImages: boolean) =>
    call<RenderedMessage>("render_message", { messageId, dark, showImages }),
  setImageSender: (email: string, allowed: boolean) => call<void>("set_image_sender", { email, allowed }),
  setPref: <K extends PrefKey>(key: K, value: Prefs[K]) => call<void>("set_pref", { key, value }),
  saveAttachment: (messageId: string, part: number, open: boolean) =>
    call<SavedAttachment>("save_attachment", { messageId, part, open }),
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
