<script lang="ts">
  import { onMount, tick } from "svelte";
  import { dismissToast } from "$lib/ui/feedback.svelte";
  import {
    Button,
    IconButton,
    Icon,
    TextField,
    Textarea,
    Select,
    Checkbox,
    Switch,
    Field,
    Card,
    Badge,
    Tabs,
    TabPanel,
    Dialog,
    toast,
  } from "$lib/ui";

  let theme = $state("system");
  let tab = $state("profile");
  let activeSection = $state("foundations");
  let name = $state("Mahasiswa ITS");
  let email = $state("student@example.com");
  let bio = $state("My classes, assignments, and campus activities.");
  let enabled = $state(true);
  let updates = $state(false);
  let density = $state("comfortable");
  let submitted = $state(false);
  let touched = $state({ name: false, email: false });
  let profileStatus = $state("");
  let notificationStatus = $state("");
  let dialog = $state(false);
  let dialogName = $state("");
  let dialogError = $state("");
  let copied = $state("");
  let tokenValues = $state<Record<string, string>>({});
  const sections = ["Foundations", "Components", "Patterns"];
  const tokens = [
    ["Background", "--background"],
    ["Text", "--foreground"],
    ["Primary", "--primary"],
    ["MIA blue", "--brand"],
    ["Secondary text", "--muted-foreground"],
    ["Border", "--border"],
  ];
  const nameError = $derived(
    (submitted || touched.name) && !name.trim()
      ? "Enter a display name."
      : undefined,
  );
  const emailError = $derived(
    (submitted || touched.email) && !/^\S+@\S+\.\S+$/.test(email)
      ? "Enter an email address, such as you@example.com."
      : undefined,
  );

  function refreshPalette() {
    const styles = getComputedStyle(document.documentElement);
    tokenValues = Object.fromEntries(
      tokens.map(([, token]) => [token, styles.getPropertyValue(token).trim()]),
    );
  }
  function setTheme(value: string) {
    theme = value;
    try {
      localStorage.setItem("mia-design-system-theme", value);
    } catch {}
    document.documentElement.dataset.theme =
      value === "system"
        ? matchMedia("(prefers-color-scheme: dark)").matches
          ? "dark"
          : "light"
        : value;
    requestAnimationFrame(refreshPalette);
  }
  onMount(() => {
    const previousTheme = document.documentElement.dataset.theme;
    let stored = "system";
    try {
      stored = localStorage.getItem("mia-design-system-theme") ?? "system";
    } catch {}
    setTheme(["system", "light", "dark"].includes(stored) ? stored : "system");
    const appearance = matchMedia("(prefers-color-scheme: dark)");
    const contrast = matchMedia("(prefers-contrast: more)");
    const forcedColors = matchMedia("(forced-colors: active)");
    const change = () => {
      if (theme === "system") {
        document.documentElement.dataset.theme = appearance.matches
          ? "dark"
          : "light";
        requestAnimationFrame(refreshPalette);
      }
    };
    const refresh = () => requestAnimationFrame(refreshPalette);
    appearance.addEventListener("change", change);
    contrast.addEventListener("change", refresh);
    forcedColors.addEventListener("change", refresh);
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries)
          if (entry.isIntersecting) activeSection = entry.target.id;
      },
      { rootMargin: "-5% 0px -65% 0px" },
    );
    document
      .querySelectorAll("main > section[id]")
      .forEach((section) => observer.observe(section));
    return () => {
      appearance.removeEventListener("change", change);
      contrast.removeEventListener("change", refresh);
      forcedColors.removeEventListener("change", refresh);
      observer.disconnect();
      dismissToast();
      if (previousTheme === undefined)
        delete document.documentElement.dataset.theme;
      else document.documentElement.dataset.theme = previousTheme;
    };
  });
  async function copy(token: string) {
    try {
      await navigator.clipboard.writeText(`var(${token})`);
      toast(`Copied ${token}`);
    } catch {
      copied = `var(${token})`;
      toast("Clipboard unavailable. Select the token below.");
    }
  }
  function resetDemo() {
    const snapshot = { name, email, bio, enabled, updates, density };
    name = "Mahasiswa ITS";
    email = "student@example.com";
    bio = "My classes, assignments, and campus activities.";
    enabled = true;
    updates = false;
    density = "comfortable";
    submitted = false;
    touched = { name: false, email: false };
    profileStatus = "";
    notificationStatus = "";
    toast("Demo preferences reset.", {
      actionLabel: "Undo",
      onAction: () => {
        ({ name, email, bio, enabled, updates, density } = snapshot);
        toast("Previous preferences restored.");
      },
    });
  }
  function editName() {
    dialogName = name;
    dialogError = "";
    dialog = true;
  }
  function applyName() {
    if (!dialogName.trim()) {
      dialogError = "Enter a display name.";
      return;
    }
    name = dialogName.trim();
    profileStatus = "";
    dialog = false;
    toast("Display name updated.");
  }
  async function save(form: HTMLFormElement) {
    submitted = true;
    if (!name.trim() || !/^\S+@\S+\.\S+$/.test(email)) {
      profileStatus = "Check the highlighted fields.";
      await tick();
      form.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus();
      return;
    }
    profileStatus = "Preferences saved for this session.";
  }
</script>

<svelte:head>
  <title>mia++ — design system</title>
  <meta
    name="description"
    content="A compact reference for the mia++ design system, built with shadcn-svelte."
  />
</svelte:head>

<a class="skip-link" href="#main-content">Skip to content</a>
<header class="masthead">
  <div class="identity">
    <h1>mia++</h1>
    <span>Design system</span>
  </div>
  <div class="toolbar">
    <nav aria-label="Sections">
      {#each sections as section}<a
          href={"#" + section.toLowerCase()}
          aria-current={activeSection === section.toLowerCase()
            ? "location"
            : undefined}
          onclick={() => (activeSection = section.toLowerCase())}>{section}</a
        >{/each}
    </nav>
    <Select
      id="preview-theme"
      class="w-auto min-w-28"
      aria-label="Preview theme"
      value={theme}
      onchange={(event) => setTheme(event.currentTarget.value)}
    >
      <option value="system">System</option><option value="light">Light</option
      ><option value="dark">Dark</option>
    </Select>
  </div>
</header>

<main id="main-content" tabindex="-1">
  <section id="foundations" aria-labelledby="foundations-heading">
    <h2 id="foundations-heading">Foundations</h2>
    <div class="palette">
      {#each tokens as [label, token]}<button
          type="button"
          class="swatch"
          aria-label={"Copy " + token}
          onclick={() => copy(token)}
        >
          <span class="color" style={"background:var(" + token + ")"}
          ></span><span>{label}</span><code>{tokenValues[token] ?? "—"}</code>
        </button>{/each}
    </div>
    {#if copied}<p class="copy-fallback">
        Copy manually: <code>{copied}</code>
      </p>{/if}
    <div class="foundation-details">
      <div>
        <h3>Typography</h3>
        <p class="type-sample">Inter</p>
        <code>System mono · code & values</code>
      </div>
      <div>
        <h3>Spacing</h3>
        <div class="spacing">
          {#each [4, 8, 12, 16, 24, 32, 48] as space}<span
              ><i style={"width:" + space / 16 + "rem"}></i><code>{space}</code
              ></span
            >{/each}
        </div>
      </div>
      <div>
        <h3>Shape</h3>
        <p class="shape-sample">
          <span>12px controls</span><span>24px panels</span>
        </p>
      </div>
    </div>
  </section>

  <section id="components" aria-labelledby="components-heading">
    <h2 id="components-heading">Components</h2>
    <div class="component-grid">
      <Card class="p-4"
        ><h3>Buttons</h3>
        <div class="examples">
          <div class="row">
            <Button onclick={() => toast("Primary action clicked.")}
              >Primary</Button
            ><Button
              variant="secondary"
              onclick={() => toast("Secondary action clicked.")}
              >Secondary</Button
            ><Button
              variant="ghost"
              onclick={() => toast("Ghost action clicked.")}>Ghost</Button
            ><Button variant="destructive" onclick={resetDemo}
              >Reset demo</Button
            >
          </div>
          <div class="row">
            <Button size="sm" onclick={() => toast("Small action clicked.")}
              >Small</Button
            ><Button onclick={() => toast("Default action clicked.")}
              >Default</Button
            ><Button size="lg" onclick={() => toast("Large action clicked.")}
              >Large</Button
            ><IconButton
              label="Show notification"
              onclick={() => toast("This is a notification.")}
              ><Icon name="bell" /></IconButton
            >
          </div>
          <div class="row">
            <Button disabled>Disabled</Button><Button loading>Loading</Button>
          </div>
        </div></Card
      >
      <Card class="p-4"
        ><h3>Fields</h3>
        <div class="examples stack">
          <Field
            id="sample-name"
            label="Input"
            description="A short hint for the field."
            ><TextField
              id="sample-name"
              placeholder="Enter a value"
              aria-describedby="sample-name-description"
            /></Field
          >
          <Field id="sample-textarea" label="Textarea"
            ><Textarea
              id="sample-textarea"
              placeholder="Write something…"
            /></Field
          >
          <TextField
            aria-label="Disabled input example"
            value="Disabled input"
            disabled
          />
        </div></Card
      >
      <Card class="p-4"
        ><h3>Selection</h3>
        <div class="examples stack">
          <Field id="density" label="Select"
            ><Select id="density" bind:value={density}
              ><option value="comfortable">Comfortable</option><option
                value="compact">Compact</option
              ></Select
            ></Field
          >
          <div class="row">
            <Checkbox bind:checked={updates} label="Checkbox" /><Checkbox
              checked
              disabled
              label="Disabled"
            />
          </div>
          <div class="row">
            <Switch bind:checked={enabled} label="Switch" /><Switch
              checked
              disabled
              label="Disabled"
            />
          </div>
        </div></Card
      >
      <Card class="p-4"
        ><h3>Badges & feedback</h3>
        <div class="examples stack">
          <div class="row">
            <Badge>Default</Badge><Badge tone="info">Available</Badge><Badge
              tone="danger">Needs attention</Badge
            >
          </div>
          <p class="muted">
            Cards group related content. Dialogs focus on one task.
          </p>
          <div class="row">
            <Button variant="secondary" onclick={editName}>Edit name…</Button
            ><Button
              variant="secondary"
              onclick={() =>
                toast("Notifications stay visible until dismissed.")}
              >Show toast</Button
            >
          </div>
        </div></Card
      >
    </div>
  </section>

  <section id="patterns" aria-labelledby="patterns-heading">
    <h2 id="patterns-heading">Settings example</h2>
    <Card class="max-w-xl p-4">
      <Tabs
        bind:value={tab}
        id="preferences"
        label="Preference categories"
        items={[
          { value: "profile", label: "Profile" },
          { value: "notifications", label: "Notifications" },
        ]}
      >
        <TabPanel
          value="profile"
          id="preferences-panel-profile"
          class="settings-body"
        >
          <form
            onsubmit={(event) => {
              event.preventDefault();
              save(event.currentTarget);
            }}
            novalidate
          >
            <div class="form-grid">
              <Field id="name" label="Display name" required error={nameError}
                ><TextField
                  id="name"
                  bind:value={name}
                  required
                  autocomplete="name"
                  onblur={() => (touched.name = true)}
                  oninput={() => (profileStatus = "")}
                  aria-invalid={!!nameError}
                  aria-describedby={nameError ? "name-error" : undefined}
                /></Field
              >
              <Field
                id="email"
                label="Email address"
                required
                error={emailError}
                ><TextField
                  id="email"
                  type="email"
                  bind:value={email}
                  required
                  autocomplete="email"
                  spellcheck={false}
                  onblur={() => (touched.email = true)}
                  oninput={() => (profileStatus = "")}
                  aria-invalid={!!emailError}
                  aria-describedby={emailError ? "email-error" : undefined}
                /></Field
              >
              <div class="full-width">
                <Field id="bio" label="About"
                  ><Textarea
                    id="bio"
                    bind:value={bio}
                    oninput={() => (profileStatus = "")}
                  /></Field
                >
              </div>
            </div>
            <div class="form-footer">
              <p class="save-status" role="status">
                {profileStatus || "Demo changes last for this session."}
              </p>
              <div class="row">
                <Button variant="ghost" onclick={resetDemo}>Reset</Button
                ><Button type="submit">Save preferences</Button>
              </div>
            </div>
          </form>
        </TabPanel>
        <TabPanel
          value="notifications"
          id="preferences-panel-notifications"
          class="settings-body"
        >
          <div class="stack">
            <Switch
              bind:checked={enabled}
              label="Desktop notifications"
            /><Checkbox bind:checked={updates} label="Product updates" />
            <div class="form-footer">
              <p class="save-status" role="status">
                {notificationStatus ||
                  "Demo controls only; no system notifications are sent."}
              </p>
              <Button
                onclick={() =>
                  (notificationStatus =
                    "Notification preferences saved for this session.")}
                >Save preferences</Button
              >
            </div>
          </div>
        </TabPanel>
      </Tabs>
    </Card>
  </section>

  <details class="usage">
    <summary>Usage example</summary>
    <pre><code
        >{`import { Button, Field, TextField, toast } from '$lib/ui';\n\n<Button onclick={() => toast('Saved.')}>Save</Button>`}</code
      ></pre>
  </details>
</main>

<Dialog
  bind:open={dialog}
  title="Edit display name"
  description="Update the name in the settings example."
>
  <form
    onsubmit={(event) => {
      event.preventDefault();
      applyName();
    }}
    novalidate
  >
    <Field
      id="dialog-name"
      label="Display name"
      required
      error={dialogError || undefined}
      ><TextField
        id="dialog-name"
        bind:value={dialogName}
        required
        autocomplete="name"
        oninput={() => (dialogError = "")}
        aria-invalid={!!dialogError}
        aria-describedby={dialogError ? "dialog-name-error" : undefined}
      /></Field
    >
    <div class="dialog-actions">
      <Button variant="secondary" onclick={() => (dialog = false)}
        >Cancel</Button
      ><Button type="submit">Apply name</Button>
    </div>
  </form>
</Dialog>

<style>
  .masthead,
  main {
    width: min(100%, 72rem);
    margin-inline: auto;
    padding-inline: 1.5rem;
  }
  .masthead {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
    padding-block: 1rem;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }
  .identity,
  .toolbar {
    display: flex;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
  }
  h1 {
    font-size: 1.25rem;
    font-weight: 600;
    letter-spacing: -0.015em;
    color: var(--primary);
  }
  .identity > span {
    color: var(--muted-foreground);
    font-size: 0.875rem;
  }
  nav {
    display: flex;
    gap: 0.25rem;
    flex-wrap: wrap;
  }
  nav a {
    padding: 0.5rem 0.625rem;
    min-height: 2.5rem;
    display: inline-flex;
    align-items: center;
    font-size: 0.8125rem;
    border-radius: var(--radius-control);
    color: var(--muted-foreground);
    text-decoration: none;
    border-bottom: 2px solid transparent;
  }
  nav a:hover {
    background: var(--muted);
    color: var(--foreground);
  }
  nav a[aria-current="location"] {
    color: var(--primary);
    background: var(--accent-soft);
    border-bottom-color: transparent;
    font-weight: 500;
  }
  main {
    padding-block: 1.5rem 2rem;
    min-width: 0;
  }
  main:focus {
    outline: none;
  }
  section {
    scroll-margin-top: 1rem;
    margin-bottom: 1.75rem;
  }
  h2 {
    font-size: 1.125rem;
    font-weight: 600;
    margin-bottom: 0.875rem;
    letter-spacing: -0.01em;
  }
  h3 {
    font-size: 0.875rem;
    font-weight: 600;
    line-height: 1.5;
  }
  .palette {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 0.75rem;
  }
  .swatch {
    display: grid;
    gap: 0.25rem;
    text-align: start;
    min-width: 0;
    background: transparent;
    border: 0;
    padding: 0;
    border-radius: var(--radius-control);
    font-size: 0.8125rem;
  }
  .color {
    height: 3rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    margin-bottom: 0.25rem;
  }
  .swatch:hover .color {
    border-color: var(--primary);
  }
  .swatch:active .color {
    outline: 2px solid var(--primary);
    outline-offset: -3px;
  }
  .swatch code {
    color: var(--muted-foreground);
    overflow-wrap: anywhere;
  }
  .foundation-details {
    display: grid;
    grid-template-columns: 1.1fr 1fr 1fr;
    gap: 1.25rem;
    padding: 1rem 0;
    border-bottom: 1px solid var(--border);
    margin-top: 1rem;
  }
  .foundation-details h3 {
    color: var(--muted-foreground);
    font-size: 0.75rem;
    margin-bottom: 0.5rem;
  }
  .type-sample {
    font-size: 1.125rem;
  }
  .foundation-details code {
    color: var(--muted-foreground);
  }
  .spacing {
    display: flex;
    align-items: start;
    gap: 0.75rem;
    flex-wrap: wrap;
  }
  .spacing > span {
    display: grid;
    gap: 0.5rem;
    justify-items: start;
  }
  .spacing i {
    display: block;
    height: 0.375rem;
    background: var(--primary);
  }
  .shape-sample {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    font-size: 0.75rem;
  }
  .shape-sample span {
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    padding: 0.5rem 0.625rem;
  }
  .shape-sample span + span {
    border-radius: var(--radius-panel);
  }
  .component-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 1rem;
  }
  .examples {
    margin-top: 1rem;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    flex-wrap: wrap;
  }
  .examples > .row + .row {
    margin-top: 0.875rem;
  }
  .stack {
    display: grid;
    gap: 0.875rem;
  }
  .muted,
  .save-status {
    color: var(--muted-foreground);
    font-size: 0.8125rem;
  }
  .form-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 1rem;
  }
  .full-width {
    grid-column: 1/-1;
  }
  :global(.settings-body) {
    padding-top: 1rem;
  }
  .form-footer {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.75rem;
    margin-top: 1rem;
  }
  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.625rem;
    flex-wrap: wrap;
    margin-top: 1.25rem;
  }
  .usage {
    font-size: 0.8125rem;
    border-top: 1px solid var(--border);
    padding-top: 1rem;
  }
  summary {
    cursor: pointer;
    color: var(--muted-foreground);
    width: fit-content;
    min-height: 2rem;
  }
  summary:hover {
    color: var(--foreground);
  }
  pre {
    background: var(--muted);
    border-radius: var(--radius-control);
    padding: 1rem;
    margin-top: 0.75rem;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .copy-fallback {
    padding: 0.5rem 0;
    font-size: 0.8125rem;
    user-select: all;
  }
  @media (max-width: 40rem) {
    .masthead,
    main {
      padding-inline: 1rem;
    }
    .toolbar {
      width: 100%;
      justify-content: space-between;
      gap: 0.5rem;
    }
    .palette {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
    .foundation-details,
    .form-grid,
    .component-grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  @media (max-width: 22rem) {
    .palette {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .toolbar {
      align-items: flex-start;
    }
    nav a {
      padding: 0.5rem;
      font-size: 0.75rem;
    }
    .dialog-actions :global(.mia-button) {
      flex: 1;
    }
  }
  @media (pointer: coarse) {
    nav a,
    summary {
      min-height: 2.75rem;
    }
  }
</style>
