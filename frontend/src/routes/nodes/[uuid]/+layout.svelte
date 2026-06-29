<script lang="ts">
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import SimpleSidebar from '@profidev/pleiades/components/nav/simple-sidebar.svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import Trash from '@lucide/svelte/icons/trash';
  import { Permission } from '$lib/permissions.svelte';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import { z } from 'zod';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { goto } from '$app/navigation';
  import { deleteNode, type NodeInfo, type UserInfo } from '$lib/client';
  import { Skeleton } from '@profidev/pleiades/components/ui/skeleton';
  import Status from '$lib/components/table/Status.svelte';

  const { children, data } = $props();

  let deleteOpen = $state(false);
  let isLoading = $state(false);
  let user: UserInfo | undefined = $state();
  let node: NodeInfo | undefined = $state();
  let readonly = $derived(!user?.permissions.includes(Permission.NODE_EDIT));

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

  $effect(() => {
    data.nodeRes.then((res) => {
      if (!res.data) {
        if (res.response?.status === 404) {
          goto('/nodes?error=not_found');
        } else {
          goto('/nodes?error=other');
        }
        return;
      }

      node = res.data;
    });
  });

  $effect(() => {
    data.user.then((d) => {
      user = d;
    });
  });

  const deleteItemConfirm = async () => {
    isLoading = true;
    let ret = await deleteNode({
      body: {
        uuid: data.uuid
      }
    });
    isLoading = false;

    if (ret) {
      return { error: 'Failed to delete node' };
    } else {
      toast.success(`Node ${node?.name} deleted successfully`);
      setTimeout(() => {
        goto('/nodes');
      });
    }
  };
</script>

<div class="flex h-full max-h-screen w-full flex-col space-y-6 p-4">
  <div class="mt-1! mb-0 ml-7 flex items-center md:m-0">
    <Button size="icon" variant="ghost" href="/nodes" class="mr-2">
      <ArrowLeft class="size-5" />
    </Button>
    <h3 class="flex text-xl font-medium text-nowrap">
      Node:
      {#if !node}
        <Skeleton class="ml-2 h-7 w-20" />
      {:else}
        {node.name}
      {/if}
    </h3>
    <Status connected={node?.connected} class="mt-0 mb-1" />
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
    class="flex min-h-0 grow flex-col space-y-4 lg:flex-row lg:space-y-0 lg:space-x-6"
  >
    <aside class="lg:w-40 lg:min-w-40">
      <SimpleSidebar items={routes} class="" />
    </aside>
    <Separator orientation="horizontal" class="lg:hidden" />
    <Separator orientation="vertical" class="hidden lg:block" />
    <div class="min-h-0 min-w-0 flex-1 grow">
      {@render children()}
    </div>
  </div>
</div>
<FormDialog
  title={`Delete Node`}
  description={`Do you really want to delete the node ${node?.name}?`}
  confirm="Delete"
  confirmVariant="destructive"
  onsubmit={deleteItemConfirm}
  bind:open={deleteOpen}
  bind:isLoading
  schema={z.object({})}
/>
