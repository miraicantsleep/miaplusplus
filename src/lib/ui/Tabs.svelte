<script lang="ts">
  import * as ShadcnTabs from "$lib/components/ui/tabs";
  import type { Snippet } from "svelte";
  let {
    value = $bindable(""),
    items,
    label = "Sections",
    id,
    children,
  }: {
    value?: string;
    items: { value: string; label: string }[];
    label?: string;
    id?: string;
    children?: Snippet;
  } = $props();
  const uid = $props.id();
  const groupId = $derived(id ?? uid);
  $effect(() => {
    if (items.length && !items.some((item) => item.value === value))
      value = items[0].value;
  });
</script>

<ShadcnTabs.Root bind:value id={groupId} loop class="gap-0">
  <ShadcnTabs.List
    variant="line"
    aria-label={label}
    class="mia-tabs-list w-full justify-start gap-6 rounded-none border-b border-border p-0"
  >
    {#each items as item (item.value)}<ShadcnTabs.Trigger
        id={groupId + "-tab-" + item.value}
        value={item.value}
        class="min-h-11 flex-none whitespace-normal px-0 py-3 text-muted-foreground data-[state=active]:text-primary after:bg-primary"
        >{item.label}</ShadcnTabs.Trigger
      >{/each}
  </ShadcnTabs.List>
  {@render children?.()}
</ShadcnTabs.Root>
