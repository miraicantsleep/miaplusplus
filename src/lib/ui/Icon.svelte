<script lang="ts">
  let { name }: { name: "bell" | "close" | "spinner" } = $props();
</script>

<svg
  class:spinning={name === "spinner"}
  width="1.125em"
  height="1.125em"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  {#if name === "bell"}<path
      d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9"
    /><path d="M10 21h4" />
  {:else if name === "close"}<path d="m6 6 12 12M18 6 6 18" />
  {:else}<circle cx="12" cy="12" r="8" opacity=".3" /><path
      d="M12 4a8 8 0 0 1 8 8"
    />{/if}
</svg>

<style>
  .spinning {
    animation: icon-spin 1s linear infinite;
    transform-origin: center;
  }
  @keyframes icon-spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spinning {
      /* Preserve loading feedback under reduced motion. */
      animation: icon-spin 1.5s linear infinite !important;
    }
  }
</style>
