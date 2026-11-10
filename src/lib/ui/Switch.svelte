<script lang="ts">
  import { Switch as ShadcnSwitch } from "$lib/components/ui/switch";
  import { Label } from "$lib/components/ui/label";
  import type { ComponentProps } from "svelte";
  let {
    checked = $bindable(false),
    label,
    id,
    ...rest
  }: ComponentProps<typeof ShadcnSwitch> & { label: string } = $props();
  const uid = $props.id();
  const controlId = $derived(id ?? uid);
</script>

<Label
  for={controlId}
  class="check min-h-10 w-fit gap-3 font-normal leading-relaxed has-disabled:opacity-50"
>
  <ShadcnSwitch {...rest} id={controlId} bind:checked />{label}
</Label>
