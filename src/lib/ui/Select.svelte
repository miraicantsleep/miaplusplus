<script lang="ts">
  import { NativeSelect } from "$lib/components/ui/native-select";
  import { cn } from "$lib/utils";
  import type { HTMLSelectAttributes } from "svelte/elements";
  import type { Snippet } from "svelte";
  let {
    value = $bindable(""),
    children,
    class: className,
    ...rest
  }: Omit<HTMLSelectAttributes, "value" | "size"> & {
    value?: string;
    children?: Snippet;
  } = $props();
</script>

<NativeSelect {...rest} bind:value class={cn("w-full", className)}
  >{@render children?.()}</NativeSelect
>
