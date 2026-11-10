<script lang="ts">
  import { Badge as ShadcnBadge } from "$lib/components/ui/badge";
  import { cn } from "$lib/utils";
  import type { ComponentProps } from "svelte";
  let {
    children,
    tone = "",
    class: className,
    ...rest
  }: Omit<ComponentProps<typeof ShadcnBadge>, "variant"> & {
    tone?: "" | "info" | "danger";
  } = $props();
</script>

<ShadcnBadge
  {...rest}
  variant={tone === "danger" ? "destructive" : "outline"}
  class={cn(
    "badge h-auto whitespace-normal rounded-md px-2 py-1 font-sans text-xs font-normal",
    tone === "info" && "border-transparent bg-accent text-primary",
    className,
  )}>{@render children?.()}</ShadcnBadge
>
