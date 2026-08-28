<script lang="ts">
  import { Button } from '@profidev/pleiades/components/ui/button';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import Plus from '@lucide/svelte/icons/plus';
  import { columns } from './table.svelte';
  import { z } from 'zod';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { afterNavigate, invalidate } from '$app/navigation';
  import { Permission } from '$lib/permissions.svelte';
  import { deleteNode, type NodeInfo, type UserInfo } from '$lib/client';
  import Table from '@profidev/pleiades/components/table/clean-table.svelte';

  const { data } = $props();

  let selected: NodeInfo | undefined = $state();
  let deleteOpen = $state(false);
  let isLoading = $state(false);
  let user: UserInfo | undefined = $state();

  let errorToasted = false;
  afterNavigate(() => {
    if (!data.error) return;

    if (!errorToasted) {
      errorToasted = true;
      if (data.error === 'node_not_found') {
        toast.error('Node not found');
      } else if (data.error === 'node_other') {
        toast.error('Failed to load node');
      }
    }

    const url = new URL(window.location.href);
    url.searchParams.delete('error');
    window.history.replaceState({}, '', url);
  });

  $effect(() => {
    data.user.then((d) => {
      user = d;
    });
  });

  const deleteItemConfirm = async () => {
    if (!selected) return;

    isLoading = true;
    let ret = await deleteNode({
      body: {
        uuid: selected.id
      }
    });
    isLoading = false;

    if (ret) {
      return { error: 'Failed to delete node' };
    } else {
      toast.success(`Node ${selected.name} deleted successfully`);
      invalidate((url) => url.pathname.startsWith('/api/nodes'));
    }
  };

  const startDeleteNode = (item: NodeInfo) => {
    selected = item;
    deleteOpen = true;
  };
</script>

<div class="p-4">
  <div class="ml-7 flex items-center md:m-0">
    <h3 class="text-xl font-medium">Nodes</h3>
    <Button
      class="ml-auto cursor-pointer"
      href="/nodes/create"
      disabled={!user?.permissions.includes(Permission.NODE_EDIT)}
    >
      <Plus />
      Create
    </Button>
  </div>
  <Table
    data={data.nodes}
    {columns}
    class="mt-4"
    columnData={{ deleteNode: startDeleteNode, user }}
  />
</div>
<FormDialog
  title={`Delete Node`}
  description={`Do you really want to delete the node ${selected?.name}?`}
  confirm="Delete"
  confirmVariant="destructive"
  onsubmit={deleteItemConfirm}
  bind:open={deleteOpen}
  bind:isLoading
  schema={z.object({})}
/>
