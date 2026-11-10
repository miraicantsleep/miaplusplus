<script lang="ts">
  import * as ShadcnField from "$lib/components/ui/field";
  import type { Snippet } from "svelte";
  let {
    label,
    id,
    description,
    error,
    required = false,
    children,
  }: {
    label: string;
    id: string;
    description?: string;
    error?: string;
    required?: boolean;
    children: Snippet;
  } = $props();
</script>

<ShadcnField.Field data-invalid={!!error} class="gap-2">
  <ShadcnField.Label for={id}>{label}{required ? " *" : ""}</ShadcnField.Label>
  {@render children()}
  {#if description}<ShadcnField.Description id={id + "-description"}
      >{description}</ShadcnField.Description
    >{/if}
  {#if error}<ShadcnField.Error id={id + "-error"}>{error}</ShadcnField.Error
    >{/if}
</ShadcnField.Field>
