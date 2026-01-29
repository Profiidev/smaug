<script lang="ts">
  import { Separator } from 'positron-components/components/ui/separator';
  import SimpleSidebar from 'positron-components/components/nav/simple-sidebar.svelte';
  import { Button } from 'positron-components/components/ui/button';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import Trash from '@lucide/svelte/icons/trash';
  import { Permission } from '$lib/permissions.svelte';
  import FormDialog from 'positron-components/components/form/form-dialog.svelte';
  import { z } from 'zod';
  import { deleteNode } from '$lib/backend/node.svelte';
  import { toast } from 'positron-components/components/util/general';
  import { goto } from '$app/navigation';

  const { children, data } = $props();

  let deleteOpen = $state(false);
  let isLoading = $state(false);
  let readonly = $derived(
    !data.user?.permissions.includes(Permission.NODE_EDIT)
  );
  const routes = $derived([
    {
      title: 'Setup',
      href: `/nodes/${data.uuid}/setup`
    },
    {
      title: 'Settings',
      href: `/nodes/${data.uuid}/settings`
    }
  ]);

  const deleteItemConfirm = async () => {
    isLoading = true;
    let ret = await deleteNode(data.node.id);
    isLoading = false;

    if (ret) {
      return { error: 'Failed to delete node' };
    } else {
      toast.success(`Node ${data.node.name} deleted successfully`);
      setTimeout(() => {
        goto('/nodes');
      });
    }
  };
</script>

<div class="flex h-full w-full flex-col space-y-6 p-4">
  <div class="mt-1! mb-0 ml-7 flex items-center md:m-0">
    <Button size="icon" variant="ghost" href="/nodes" class="mr-2">
      <ArrowLeft class="size-5" />
    </Button>
    <h3 class="text-xl font-medium">Node: {data.node.name}</h3>
    <Button
      class="ml-auto cursor-pointer"
      onclick={() => (deleteOpen = true)}
      variant="destructive"
      disabled={readonly}
    >
      <Trash />
      Delete
    </Button>
  </div>
  <Separator class="my-4" />
  <div
    class="flex grow flex-col space-y-4 lg:flex-row lg:space-y-0 lg:space-x-6"
  >
    <aside class="lg:w-40">
      <SimpleSidebar items={routes} class="" />
    </aside>
    <Separator orientation="horizontal" class="lg:hidden" />
    <Separator orientation="vertical" class="hidden lg:block" />
    <div class="flex-1">
      {@render children()}
    </div>
  </div>
</div>
<FormDialog
  title={`Delete Node`}
  description={`Do you really want to delete the node ${data.node.name}?`}
  confirm="Delete"
  confirmVariant="destructive"
  onsubmit={deleteItemConfirm}
  bind:open={deleteOpen}
  bind:isLoading
  schema={z.object({})}
/>
