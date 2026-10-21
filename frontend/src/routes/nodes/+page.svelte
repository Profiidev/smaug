<script lang="ts">
  import { Button } from '@profidev/pleiades/components/ui/button';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import Plus from '@lucide/svelte/icons/plus';
  import Server from '@lucide/svelte/icons/server';
  import Table from '@profidev/pleiades/components/table/clean-table.svelte';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { invalidate } from '$app/navigation';
  import { z } from 'zod';
  import { Permission } from '$lib/permissions.svelte';
  import { deleteNode, type NodeInfo, type UserInfo } from '$lib/client';
  import { columns } from './table.svelte';
  import { DELETE_DESCRIPTION, deleteError } from './schema.svelte';

  const { data } = $props();

  const ERRORS: Record<string, string> = {
    not_found: 'Node not found',
    other: 'Failed to load node'
  };

  let selected: NodeInfo | undefined = $state();
  let deleteOpen = $state(false);
  let isLoading = $state(false);
  let user: UserInfo | undefined = $state();
  // Undefined while loading; drives the table skeleton, count and empty state.
  let nodes: NodeInfo[] | undefined = $state();

  let canEdit = $derived(
    user?.permissions.includes(Permission.NODE_EDIT) ?? false
  );

  $effect(() => {
    data.nodes.then((d) => {
      nodes = d ?? [];
    });
  });

  $effect(() => {
    data.user.then((d) => {
      user = d;
    });
  });

  $effect(() => {
    if (data.error) {
      if (Object.hasOwn(ERRORS, data.error)) {
        toast.error(ERRORS[data.error]);
      }

      const url = new URL(window.location.href);
      url.searchParams.delete('error');
      window.history.replaceState({}, '', url);
    }
  });

  const deleteItemConfirm = async () => {
    if (!selected) return;

    isLoading = true;
    const res = await deleteNode({ body: { uuid: selected.id } });
    isLoading = false;

    if (!res.response?.ok) {
      return { error: deleteError(res.response?.status) };
    }

    toast.success(`Node ${selected.name} deleted successfully`);
    invalidate((url) => url.pathname.startsWith('/api/nodes'));
  };

  const startDelete = (item: NodeInfo) => {
    selected = item;
    deleteOpen = true;
  };
</script>

<div class="flex flex-col gap-4 p-4">
  <div class="ml-7 flex flex-wrap items-start gap-3 md:m-0">
    <div class="grid gap-1">
      <h3 class="text-xl font-medium">Nodes</h3>
      <p class="text-muted-foreground text-sm">
        Machines running Wings that host your servers.
      </p>
    </div>
    <!-- The empty state carries the same action; avoid duplicate controls. -->
    {#if nodes?.length !== 0}
      <div class="ml-auto">
        {@render create()}
      </div>
    {/if}
  </div>
  {#if nodes?.length === 0}
    <Empty.Root class="border border-dashed">
      <Empty.Header>
        <Empty.Media variant="icon"><Server /></Empty.Media>
        <Empty.Title>No nodes yet</Empty.Title>
        <Empty.Description>
          Add a node, then run Wings on it with the setup command.
        </Empty.Description>
      </Empty.Header>
      <Empty.Content class="flex-row justify-center">
        {@render create()}
      </Empty.Content>
    </Empty.Root>
  {:else}
    <div class="grid gap-2">
      <Table
        data={nodes}
        {columns}
        columnData={{ canEdit, deleteNode: startDelete }}
        searchColumns={['name', 'address', 'id']}
      />
      {#if nodes}
        <span class="text-muted-foreground px-1 text-xs">
          {nodes.length}
          {nodes.length === 1 ? 'node' : 'nodes'}
        </span>
      {/if}
    </div>
  {/if}
</div>

{#snippet create()}
  <Button class="cursor-pointer" href="/nodes/create" disabled={!canEdit}>
    <Plus />
    Create
  </Button>
{/snippet}

<FormDialog
  title={`Delete node ${selected?.name}?`}
  description={DELETE_DESCRIPTION}
  confirm="Delete"
  confirmVariant="destructive"
  onsubmit={deleteItemConfirm}
  bind:open={deleteOpen}
  bind:isLoading
  schema={z.object({})}
/>
