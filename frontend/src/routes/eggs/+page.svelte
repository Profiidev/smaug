<script lang="ts">
  import { Button } from '@profidev/pleiades/components/ui/button';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import Plus from '@lucide/svelte/icons/plus';
  import Table from '@profidev/pleiades/components/table/clean-table.svelte';
  import { columns, type EggListItem } from './table.svelte';
  import { z } from 'zod';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { invalidate } from '$app/navigation';
  import { Permission } from '$lib/permissions.svelte';
  import type { UserInfo } from '$lib/client';

  const { data } = $props();

  let selected: EggListItem | undefined = $state();
  let deleteOpen = $state(false);
  let isLoading = $state(false);
  let user: UserInfo | undefined = $state();

  let canEdit = $derived(
    user?.permissions.includes(Permission.EGG_EDIT) ?? false
  );

  $effect(() => {
    data.user.then((d) => {
      user = d;
    });
  });

  $effect(() => {
    if (data.error) {
      if (data.error === 'not_found') {
        toast.error('Egg not found');
      } else if (data.error === 'other') {
        toast.error('Failed to load egg');
      }

      const url = new URL(window.location.href);
      url.searchParams.delete('error');
      window.history.replaceState({}, '', url);
    }
  });

  const deleteItemConfirm = async () => {
    if (!selected) return;

    isLoading = true;
    // TODO: replace with deleteEgg({ body: { uuid: selected.uuid } }) from $lib/client
    // once backend egg routes exist.
    isLoading = false;

    toast.success(`Egg ${selected.name} deleted successfully`);
    invalidate((url) => url.pathname.startsWith('/api/eggs'));
  };

  const startDeleteEgg = (item: EggListItem) => {
    selected = item;
    deleteOpen = true;
  };
</script>

<div class="p-4">
  <div class="ml-7 flex items-center md:m-0">
    <h3 class="text-xl font-medium">Eggs</h3>
    <Button
      class="ml-auto cursor-pointer"
      href="/eggs/create"
      disabled={!canEdit}
    >
      <Plus />
      Create
    </Button>
  </div>
  <Table
    data={data.eggs}
    {columns}
    class="mt-4"
    columnData={{ canEdit, deleteEgg: startDeleteEgg }}
    searchColumns={['name', 'author', 'description']}
  />
</div>
<FormDialog
  title={`Delete Egg`}
  description={`Do you really want to delete the egg ${selected?.name}?`}
  confirm="Delete"
  confirmVariant="destructive"
  onsubmit={deleteItemConfirm}
  bind:open={deleteOpen}
  bind:isLoading
  schema={z.object({})}
/>
