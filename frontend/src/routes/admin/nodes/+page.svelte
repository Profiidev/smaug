<script lang="ts">
  import { Button } from 'positron-components/components/ui/button';
  import FormDialog from 'positron-components/components/form/form-dialog.svelte';
  import Plus from '@lucide/svelte/icons/plus';
  import type { PageData } from './$types';
  import Table from '$lib/components/table/Table.svelte';
  import { columns } from './table.svelte';
  import { deleteNode, type NodeInfo } from '$lib/backend/node.svelte';
  import { z } from 'zod';
  import { toast } from 'positron-components/components/util/general';
  import { invalidate } from '$app/navigation';

  interface Props {
    data: PageData;
  }

  const { data }: Props = $props();

  let selected: NodeInfo | undefined = $state();
  let deleteOpen = $state(false);
  let isLoading = $state(false);

  const deleteItemConfirm = async () => {
    if (!selected) return;

    isLoading = true;
    let ret = await deleteNode(selected.id);
    isLoading = false;

    if (ret) {
      return { error: 'Failed to delete node' };
    } else {
      toast.success(`Node ${selected.name} deleted successfully`);
      invalidate((url) => url.pathname.startsWith('/api/admin/nodes'));
    }
  };

  const startDeleteNode = (item: NodeInfo) => {
    selected = item;
    deleteOpen = true;
  };
</script>

<div class="p-4">
  <div class="flex items-center ml-7 md:m-0">
    <h3 class="text-xl font-medium">Nodes</h3>
    <Button class="ml-auto" href="/admin/nodes/create">
      <Plus />
      Create
    </Button>
  </div>
  <Table
    data={data.nodes}
    {columns}
    class="mt-4"
    columnData={{ deleteNode: startDeleteNode }}
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
