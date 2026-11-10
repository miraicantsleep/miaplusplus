<script lang="ts">
  import * as ShadcnDialog from "$lib/components/ui/dialog";
  import type { Snippet } from "svelte";
  let {
    open = $bindable(false),
    title,
    description,
    children,
  }: {
    open?: boolean;
    title: string;
    description?: string;
    children?: Snippet;
  } = $props();
  let opener: HTMLElement | null = null;
</script>

<ShadcnDialog.Root bind:open>
  <ShadcnDialog.Content
    class="max-h-[calc(100dvh-2rem)] overflow-y-auto p-7 sm:max-w-[27.5rem]"
    onOpenAutoFocus={() => {
      opener =
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
    }}
    onCloseAutoFocus={(event) => {
      if (opener?.isConnected) {
        event.preventDefault();
        opener.focus();
      }
      opener = null;
    }}
  >
    <ShadcnDialog.Header class="gap-3 pr-10 text-start">
      <ShadcnDialog.Title class="text-2xl font-semibold"
        >{title}</ShadcnDialog.Title
      >
      <ShadcnDialog.Description class={description ? "" : "sr-only"}
        >{description ?? title}</ShadcnDialog.Description
      >
    </ShadcnDialog.Header>
    <div class="pt-1">{@render children?.()}</div>
  </ShadcnDialog.Content>
</ShadcnDialog.Root>
