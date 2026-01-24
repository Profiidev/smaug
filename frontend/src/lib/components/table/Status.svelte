<script lang="ts">
  import * as Tooltip from 'positron-components/components/ui/tooltip';
  import HeartPulse from '@lucide/svelte/icons/heart-pulse';
  import HeartCrack from '@lucide/svelte/icons/heart-crack';

  interface Props {
    connected: boolean;
  }

  const { connected }: Props = $props();
</script>

<Tooltip.Provider>
  <Tooltip.Root>
    <Tooltip.Trigger
      class={`mt-1 ml-3 ${connected ? 'text-green-500' : 'text-red-500'}`}
    >
      {#if connected}
        <HeartPulse />
      {:else}
        <HeartCrack />
      {/if}
    </Tooltip.Trigger>
    <Tooltip.Content>
      {#if !connected}
        <p>Check the server logs for more information.</p>
      {/if}
    </Tooltip.Content>
  </Tooltip.Root>
</Tooltip.Provider>
