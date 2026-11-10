<script lang="ts">
  import { onMount } from "svelte";
  import { invoke, isTauri } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { Button } from "$lib/ui";
  type Phase = "checking" | "signing_in" | "signed_in" | "signed_out" | "error";
  type AuthStatus = {
    state: Phase;
    revision: number;
    name?: string | null;
    username?: string | null;
    message?: string | null;
  };
  type AuthCommand =
    | "auth_get_status"
    | "auth_start_login"
    | "auth_cancel_login"
    | "auth_logout";
  let status = $state<AuthStatus>({ state: "checking", revision: 0 });
  let message = $state("");
  let native = $state(false);
  const busy = $derived(
    status.state === "checking" || status.state === "signing_in",
  );
  function apply(next: AuthStatus) {
    // IPC replies may arrive after a more recent auth:changed event.
    if (next.revision >= status.revision) {
      status = next;
      message = "";
    }
  }
  async function action(command: AuthCommand) {
    message = "";
    if (!native) {
      message =
        "Open the desktop app to sign in and securely remember your ITS account.";
      return;
    }
    try {
      apply(await invoke<AuthStatus>(command));
    } catch (error) {
      if (command === "auth_get_status") status = { ...status, state: "error" };
      message =
        typeof error === "string"
          ? error
          : "Could not complete this action. Please retry.";
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
    let disposed = false;
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
          status = { ...status, state: "error" };
          message = "Could not connect to the desktop app. Please restart it.";
        });
    } else status = { state: "signed_out", revision: 0 };
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
    {#if status.state === "signed_in"}
      <h1 id="login-heading">You're signed in</h1>
      <p>{status.name || status.username}</p>
      <p class="description">Your ITS account is connected.</p>
      <Button
        class="w-full"
        variant="secondary"
        onclick={() => action("auth_logout")}>Log out</Button
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
          onclick={() => action("auth_logout")}>Reset saved sign-in</Button
        >{/if}
      {#if status.state === "signing_in"}<Button
          class="w-full"
          variant="ghost"
          onclick={() => action("auth_cancel_login")}>Cancel sign-in</Button
        >{/if}
      <p class="hint">absolutely not affiliated with ITS</p>
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
