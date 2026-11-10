<script lang="ts">
  import { Card as ShadcnCard } from "$lib/components/ui/card";
  import { cn } from "$lib/utils";
  import type { ComponentProps } from "svelte";
  let {
    children,
    tone = "",
    class: className,
    ...rest
  }: ComponentProps<typeof ShadcnCard> & {
    tone?: "" | "info" | "danger";
  } = $props();
</script>

<ShadcnCard
  {...rest}
  class={cn(
    "card block min-w-0 gap-0 overflow-visible border border-border p-6 text-base shadow-none ring-0",
    tone === "info" && "bg-accent",
    tone === "danger" && "border-destructive",
    className,
  )}>{@render children?.()}</ShadcnCard
>
