<script lang="ts">
  import {
    Button as ShadcnButton,
    type ButtonProps,
  } from "$lib/components/ui/button";
  import { cn } from "$lib/utils";
  import Icon from "./Icon.svelte";
  let {
    variant = "primary",
    size = "md",
    loading = false,
    children,
    class: className,
    ...rest
  }: Omit<ButtonProps, "variant" | "size"> & {
    variant?: "primary" | "secondary" | "ghost" | "destructive";
    size?: "sm" | "md" | "lg" | "icon";
    loading?: boolean;
  } = $props();
  const variants = {
    primary: "default",
    secondary: "outline",
    ghost: "ghost",
    destructive: "destructive",
  } as const;
  const sizes = { sm: "sm", md: "default", lg: "lg", icon: "icon" } as const;
</script>

<ShadcnButton
  {...rest}
  variant={variants[variant]}
  size={sizes[size]}
  disabled={rest.disabled || loading}
  aria-busy={loading}
  class={cn(
    "mia-button h-auto min-h-10 gap-2 whitespace-normal rounded-[var(--radius-button)] font-semibold px-4 py-2.5",
    size === "sm" && "min-h-8 px-3 py-1.5 text-[.8125rem]",
    size === "lg" && "min-h-12 px-5 py-3",
    size === "icon" && "size-10 min-w-10 p-2.5",
    variant === "secondary" && "border-input bg-card dark:bg-card",
    className,
  )}
>
  {#if loading}<span class="spinner" aria-hidden="true"
      ><Icon name="spinner" /></span
    >{/if}{@render children?.()}
</ShadcnButton>
