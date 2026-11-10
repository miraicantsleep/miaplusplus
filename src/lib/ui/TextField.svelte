<script lang="ts">
  import { Input } from "$lib/components/ui/input";
  import { cn } from "$lib/utils";
  import type {
    HTMLInputAttributes,
    HTMLInputTypeAttribute,
  } from "svelte/elements";
  let {
    value = $bindable(""),
    class: className,
    ...rest
  }: Omit<HTMLInputAttributes, "value" | "type" | "files"> & {
    value?: string;
    type?: Exclude<HTMLInputTypeAttribute, "file">;
  } = $props();
</script>

<Input
  {...rest}
  bind:value
  class={cn(
    "mia-control h-auto min-h-11 rounded-md bg-background px-3 py-2.5 text-sm dark:bg-background",
    className,
  )}
/>
