<script lang="ts">
  import Egg from '@lucide/svelte/icons/egg';
  import { Badge } from '@profidev/pleiades/components/ui/badge';

  interface Props {
    name: string;
    href: string;
    /** Set when an update check found a newer source; opens the review dialog. */
    onreview?: () => void;
    /** Reason the last update check failed. */
    failed?: string;
  }

  let { name, href, onreview, failed }: Props = $props();
</script>

<div class="ml-4 flex items-center gap-2.5">
  <div
    class="bg-muted text-muted-foreground flex size-7 flex-none items-center justify-center rounded-lg"
  >
    <Egg class="size-4" />
  </div>
  <a {href} class="font-medium underline-offset-4 hover:underline">{name}</a>
  {#if onreview}
    <button
      type="button"
      class="cursor-pointer"
      aria-label="Review update for {name}"
      onclick={onreview}
    >
      <Badge>Update available</Badge>
    </button>
  {:else if failed}
    <Badge variant="destructive" title={failed}>Check failed</Badge>
  {/if}
</div>
