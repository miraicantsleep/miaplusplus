<script lang="ts">
  import { Textarea as ShadcnTextarea } from "$lib/components/ui/textarea";
  import { cn } from "$lib/utils";
  import type { HTMLTextareaAttributes } from "svelte/elements";
  let {
    value = $bindable(""),
    class: className,
    ...rest
  }: Omit<HTMLTextareaAttributes, "value" | "children"> & {
    value?: string;
  } = $props();
</script>

<ShadcnTextarea
  {...rest}
  bind:value
  class={cn(
    "mia-control min-h-24 resize-y rounded-md bg-background px-3 py-2.5 text-sm dark:bg-background",
    className,
  )}
/>
