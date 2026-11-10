<script lang="ts">
  import { Checkbox as ShadcnCheckbox } from "$lib/components/ui/checkbox";
  import { Label } from "$lib/components/ui/label";
  import type { ComponentProps } from "svelte";
  let {
    checked = $bindable(false),
    label,
    id,
    ...rest
  }: ComponentProps<typeof ShadcnCheckbox> & { label: string } = $props();
  const uid = $props.id();
  const controlId = $derived(id ?? uid);
</script>

<Label
  for={controlId}
  class="check min-h-10 w-fit gap-2.5 font-normal leading-relaxed has-disabled:opacity-50"
>
  <ShadcnCheckbox
    {...rest}
    id={controlId}
    bind:checked
    class="size-[1.125rem] rounded-[4px]"
  />{label}
</Label>
