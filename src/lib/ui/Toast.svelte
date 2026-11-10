<script lang="ts">
  import { Toaster } from "$lib/components/ui/sonner";
</script>

<Toaster
  theme="system"
  position="bottom-right"
  duration={Infinity}
  closeButton
  visibleToasts={1}
  closeButtonAriaLabel="Dismiss notification"
/>
