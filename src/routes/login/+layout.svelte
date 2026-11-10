<script lang="ts">
  import shadcnStyles from "$lib/styles/shadcn.css?inline";
  import systemStyles from "$lib/styles/system.css?inline";
  let { children } = $props();
</script>

<svelte:head
  >{@html `<style>${shadcnStyles}</style><style>${systemStyles}</style>`}</svelte:head
>
{@render children()}
