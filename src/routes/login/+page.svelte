<script lang="ts">
  import { onMount } from "svelte";
  import { invoke, isTauri } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { Badge, Button, Dialog, Icon } from "$lib/ui";
  type Phase =
    | "checking"
    | "signing_in"
    | "signing_out"
    | "signed_in"
    | "signed_out"
    | "error";
  type AuthStatus = {
    state: Phase;
    revision: number;
    name?: string | null;
    username?: string | null;
    message?: string | null;
    apps: {
      app: "mia" | "classroom" | "kemahasiswaan";
      state: "checking" | "connected" | "failed";
      message?: string | null;
    }[];
  };
  type AuthCommand =
    | "auth_get_status"
    | "auth_start_login"
    | "auth_cancel_login"
    | "auth_logout"
    | "auth_connect_apps"
    | "auth_forget_session";
  let status = $state<AuthStatus>({ state: "checking", revision: 0, apps: [] });
  const appNames = {
    mia: "MIA",
    classroom: "Classroom",
    kemahasiswaan: "Kemahasiswaan",
  };
  const connectionLabels = {
    checking: "Connecting...",
    connected: "Authenticated",
    failed: "Connection failed",
  };
  const connecting = $derived(
    status.apps.some((app) => app.state === "checking"),
  );
  let message = $state("");
  let native = $state(false);
  let pending = $state(false);
  let logoutOpen = $state(false);
  let disposed = false;
  let request = 0;
  const busy = $derived(
    pending || ["checking", "signing_in", "signing_out"].includes(status.state),
  );
  function apply(next: AuthStatus) {
    // Ignore stale IPC replies.
    if (!disposed && next.revision >= status.revision) {
      status = next;
      message = "";
      if (next.state !== "signed_in") logoutOpen = false;
    }
  }
  async function action(command: AuthCommand) {
    if (pending || disposed) return;
    message = "";
    if (!native) {
      message =
        "Open the desktop app to sign in and securely remember your ITS account.";
      return;
    }
    pending = true;
    const attempt = ++request;
    const revision = status.revision;
    try {
      const next = await invoke<AuthStatus>(command);
      if (attempt === request) apply(next);
    } catch (error) {
      if (disposed || attempt !== request || status.revision > revision) return;
      if (command === "auth_get_status") status = { ...status, state: "error" };
      message =
        typeof error === "string"
          ? error
          : "Could not complete this action. Please retry.";
    } finally {
      if (attempt === request) pending = false;
    }
  }
  onMount(() => {
    const root = document.documentElement;
    const previous = root.dataset.theme;
    const appearance = matchMedia("(prefers-color-scheme: dark)");
    const sync = () =>
      (root.dataset.theme = appearance.matches ? "dark" : "light");
    sync();
    appearance.addEventListener("change", sync);
    native = isTauri();
    disposed = false;
    let unlisten: (() => void) | undefined;
    if (native) {
      listen<AuthStatus>("auth:changed", (e) => {
        apply(e.payload);
      })
        .then((stop) => {
          if (disposed) stop();
          else {
            unlisten = stop;
            void action("auth_get_status");
          }
        })
        .catch(() => {
          if (disposed) return;
          status = { ...status, state: "error" };
          message = "Could not connect to the desktop app. Please restart it.";
        });
    } else status = { state: "signed_out", revision: 0, apps: [] };
    return () => {
      disposed = true;
      unlisten?.();
      appearance.removeEventListener("change", sync);
      if (previous === undefined) delete root.dataset.theme;
      else root.dataset.theme = previous;
    };
  });
</script>

<svelte:head
  ><title>Login - MIA++</title><meta
    name="description"
    content="Sign in to MIA++ with your ITS account."
  /></svelte:head
>
<main>
  <section aria-labelledby="login-heading">
    {#if status.state === "signed_in" || status.state === "signing_out"}
      <h1 id="login-heading">You're signed in</h1>
      <p>{status.name || status.username}</p>
      <p class="description">Your ITS account is connected.</p>
      <ul
        class="mb-6 divide-y divide-border text-left text-sm"
        aria-label="ITS app connections"
        aria-live="polite"
      >
        {#each status.apps as app (app.app)}
          <li class="flex flex-wrap items-center justify-between gap-2 py-3">
            <span>{appNames[app.app]}</span>
            <Badge
              tone={app.state === "failed"
                ? "danger"
                : app.state === "connected"
                  ? "info"
                  : ""}
            >
              {#if app.state === "checking"}<span class="spinner"
                  ><Icon name="spinner" /></span
                >{/if}
              {connectionLabels[app.state]}
            </Badge>
            {#if app.message}<p
                class="w-full text-xs leading-relaxed text-muted-foreground"
              >
                {app.message}
              </p>{/if}
          </li>
        {/each}
      </ul>
      {#if status.apps.some((app) => app.state === "failed") && status.state === "signed_in"}
        <Button
          class="w-full"
          variant="ghost"
          disabled={connecting}
          onclick={() => action("auth_connect_apps")}>Retry connections</Button
        >
      {/if}
      <Button
        class="w-full"
        variant="secondary"
        disabled={busy}
        loading={status.state === "signing_out"}
        onclick={() => (logoutOpen = true)}
        >{status.state === "signing_out" ? "Signing out..." : "Log out"}</Button
      >
    {:else}
      <h1 id="login-heading">Welcome to MIA++</h1>
      <p class="description">unified app for all ITS student things</p>
      <Button
        class="w-full"
        disabled={busy}
        loading={busy}
        onclick={() => action("auth_start_login")}
        >Login with an ITS student account</Button
      >
      {#if status.state === "error" && native}<Button
          variant="ghost"
          class="w-full"
          onclick={() => action("auth_forget_session")}
          >Reset saved sign-in</Button
        >{/if}
      {#if status.state === "signing_in"}<Button
          class="w-full"
          variant="ghost"
          onclick={() => action("auth_cancel_login")}>Cancel sign-in</Button
        >{/if}
      <p class="hint">Not affiliated with ITS</p>
    {/if}
    <p class="status" role="status" aria-live="polite">
      {message ||
        status.message ||
        (status.state === "checking"
          ? "Checking your saved session..."
          : status.state === "signing_in"
            ? "Continue in the ITS sign-in window."
            : "")}
    </p>
  </section>
</main>

<Dialog
  bind:open={logoutOpen}
  title="Log out of MIA++?"
  description="This ends your current myITS SSO session and removes your saved sign-in from this device."
>
  <div class="flex flex-wrap justify-end gap-3 pt-4">
    <Button variant="secondary" onclick={() => (logoutOpen = false)}
      >Cancel</Button
    >
    <Button
      variant="destructive"
      disabled={busy}
      onclick={() => {
        logoutOpen = false;
        void action("auth_logout");
      }}>Log out</Button
    >
  </div>
</Dialog>

<style>
  main {
    min-height: 100dvh;
    display: grid;
    place-items: center;
    padding: 1.5rem;
  }
  section {
    width: 100%;
    max-width: 25rem;
    text-align: center;
  }
  h1 {
    font-size: 1.5rem;
  }
  .description {
    color: var(--muted-foreground);
    margin: 0.75rem 0 1.75rem;
  }
  .hint,
  .status {
    font-size: 0.8125rem;
    color: var(--muted-foreground);
    line-height: 1.5;
    margin-top: 1rem;
  }
  .status:empty {
    display: none;
  }
  @media (max-width: 30rem) {
    main {
      padding: 1rem;
    }
  }
</style>
