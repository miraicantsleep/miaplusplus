<script lang="ts">
  import shadcnStyles from "$lib/styles/shadcn.css?inline";
  import systemStyles from "$lib/styles/system.css?inline";
  import { Toast } from "$lib/ui";
  let { children } = $props();
</script>

<svelte:head>
  <!-- Route-owned styles are removed when leaving the showcase. -->
  {@html `<style>${shadcnStyles}</style><style>${systemStyles}</style>`}
  <link
    rel="preload"
    href="/fonts/inter-latin.woff2"
    as="font"
    type="font/woff2"
    crossorigin="anonymous"
  />
</svelte:head>

{@render children()}<Toast />
