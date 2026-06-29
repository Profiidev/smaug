<script lang="ts">
  import * as Tooltip from '@profidev/pleiades/components/ui/tooltip';
  import HeartPulse from '@lucide/svelte/icons/heart-pulse';
  import HeartCrack from '@lucide/svelte/icons/heart-crack';
  import Heart from '@lucide/svelte/icons/heart';
  import { cn } from '@profidev/pleiades/utils';

  interface Props {
    connected?: boolean;
    class?: string;
  }

  const { connected, class: className }: Props = $props();
</script>

<Tooltip.Provider>
  <Tooltip.Root>
    <Tooltip.Trigger
      class={cn(
        `mt-1 ml-3 ${connected === undefined ? '' : connected ? 'text-green-500' : 'text-red-500'}`,
        className
      )}
    >
      {#if connected === undefined}
        <Heart />
      {:else if connected}
        <HeartPulse />
      {:else}
        <HeartCrack />
      {/if}
    </Tooltip.Trigger>
    <Tooltip.Content>
      {#if connected === undefined}
        <p>Status loading...</p>
      {:else if !connected}
        <p>Check the server logs for more information.</p>
      {:else}
        <p>Connection is healthy.</p>
      {/if}
    </Tooltip.Content>
  </Tooltip.Root>
</Tooltip.Provider>
