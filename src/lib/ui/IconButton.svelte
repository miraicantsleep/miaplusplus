<script lang="ts">
  import Button from "./Button.svelte";
  import type { ComponentProps } from "svelte";
  let {
    label,
    children,
    ...rest
  }: Omit<ComponentProps<typeof Button>, "variant" | "size"> & {
    label: string;
  } = $props();
</script>

<Button
  {...rest}
  variant="ghost"
  size="icon"
  title={rest.title ?? label}
  aria-label={label}>{@render children?.()}</Button
>
