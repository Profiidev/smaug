<script lang="ts">
  import { Separator } from 'positron-components/components/ui/separator';
  import { Button } from 'positron-components/components/ui/button';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import Trash from '@lucide/svelte/icons/trash';
  import { Permission } from '$lib/permissions.svelte';
  import FormDialog from 'positron-components/components/form/form-dialog.svelte';
  import { z } from 'zod';
  import { toast } from 'positron-components/components/util/general';
  import { goto } from '$app/navigation';
  import { deleteGroup } from '$lib/backend/groups.svelte.js';

  const { data } = $props();

  let deleteOpen = $state(false);
  let isLoading = $state(false);
  let readonly = $derived(
    !data.user?.permissions.includes(Permission.GROUP_EDIT)
  );
  const deleteItemConfirm = async () => {
    isLoading = true;
    let ret = await deleteGroup({ uuid: data.group.id });
    isLoading = false;

    if (ret) {
      return { error: 'Failed to delete group' };
    } else {
      toast.success(`Group ${data.group.name} deleted successfully`);
      setTimeout(() => {
        goto('/groups');
      });
    }
  };
</script>

<div class="flex h-full w-full flex-col space-y-6 p-4">
  <div class="mt-1! mb-0 ml-7 flex items-center md:m-0">
    <Button size="icon" variant="ghost" href="/groups" class="mr-2">
      <ArrowLeft class="size-5" />
    </Button>
    <h3 class="text-xl font-medium">Group: {data.group.name}</h3>
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
    <p>Edit Form</p>
  </div>
</div>
<FormDialog
  title={`Delete Group`}
  description={`Do you really want to delete the group ${data.group.name}?`}
  confirm="Delete"
  confirmVariant="destructive"
  onsubmit={deleteItemConfirm}
  bind:open={deleteOpen}
  bind:isLoading
  schema={z.object({})}
/>
