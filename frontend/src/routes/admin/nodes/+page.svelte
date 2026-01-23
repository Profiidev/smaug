<script lang="ts">
  import { Button } from 'positron-components/components/ui/button';
  import Plus from '@lucide/svelte/icons/plus';
  import type { PageData } from './$types';

  interface Props {
    data: PageData;
  }

  const { data }: Props = $props();
</script>

<div class="p-4">
  <div class="flex items-center">
    <h3 class="text-xl font-medium">Nodes</h3>
    <Button class="ml-auto" href="/admin/nodes/create">
      <Plus />
      Create
    </Button>
  </div>
  {#await data.nodes}
    <p>Loading...</p>
  {:then nodes}
    {#if nodes?.length === 0}
      <p>No nodes found.</p>
    {/if}
    {#each nodes as node}
      <p>
        {node.name}
        {node.token}
        {node.connected ? 'Connected' : 'Disconnected'}
      </p>
    {/each}
  {/await}
</div>
