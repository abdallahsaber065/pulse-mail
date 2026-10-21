import { createSignal, Show } from "solid-js";
import Icon from "./Icon";

interface Props {
  clientConfigured: boolean;
  clientConfigPath: string;
  error: string | null;
  onSignIn(): Promise<void>;
  onRecheck(): void;
}

export default function SetupCard(p: Props) {
  const [busy, setBusy] = createSignal(false);
  const signIn = async () => {
    setBusy(true);
    try {
      await p.onSignIn();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div class="grid h-full place-items-center p-6">
      <div class="w-full max-w-lg rounded-3xl bg-surface p-8 shadow-sm">
        <div class="mb-4 grid size-12 place-items-center rounded-2xl bg-primary text-on-primary">
          <Icon name="mail" size={28} />
        </div>
        <h1 class="mb-2 font-display text-2xl text-text">Welcome to Pulse Mail</h1>
        <Show
          when={p.clientConfigured}
          fallback={
            <>
              <p class="mb-4 text-sm leading-6 text-muted">
                Pulse Mail signs in with your own Google Cloud OAuth client. Create a Desktop-app client (see
                <span class="font-medium text-text"> docs/SETUP-GOOGLE-OAUTH.md</span>), download its JSON, and save it as:
              </p>
              <code class="selectable mb-5 block break-all rounded-lg bg-surface-container px-3 py-2 font-mono text-xs text-text">
                {p.clientConfigPath}
              </code>
              <button class="rounded-full bg-primary px-6 py-2.5 text-sm font-medium text-on-primary hover:shadow" onClick={p.onRecheck}>
                I've saved it
              </button>
            </>
          }
        >
          <p class="mb-6 text-sm leading-6 text-muted">
            Sign in with Google to get new mail and verification codes the moment they arrive. Your sign-in is kept in Windows
            Credential Manager on this PC only.
          </p>
          <button
            class="flex items-center gap-2 rounded-full bg-primary px-6 py-2.5 text-sm font-medium text-on-primary hover:shadow disabled:opacity-60"
            onClick={signIn}
            disabled={busy()}
          >
            {busy() ? "Finish signing in in your browser…" : "Sign in with Google"}
          </button>
        </Show>
        <Show when={p.error}>
          <p class="mt-4 text-sm text-danger" role="alert">{p.error}</p>
        </Show>
      </div>
    </div>
  );
}
