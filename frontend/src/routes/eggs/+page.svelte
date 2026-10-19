<script lang="ts">
  import { Button } from '@profidev/pleiades/components/ui/button';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import Plus from '@lucide/svelte/icons/plus';
  import Table from '@profidev/pleiades/components/table/clean-table.svelte';
  import { columns } from './table.svelte';
  import { z } from 'zod';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { goto, invalidate } from '$app/navigation';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import Egg from '@lucide/svelte/icons/egg';
  import ImportButton from './ImportButton.svelte';
  import { Permission } from '$lib/permissions.svelte';
  import { deleteEgg, type EggListItem, type UserInfo } from '$lib/client';
  import { apiErrorMessage, type EggImport } from './schema.svelte';

  const { data } = $props();

  let selected: EggListItem | undefined = $state();
  let deleteOpen = $state(false);
  let isLoading = $state(false);
  let user: UserInfo | undefined = $state();
  // Undefined while loading; drives the count caption and the empty state.
  let eggs: EggListItem[] | undefined = $state();

  let canEdit = $derived(
    user?.permissions.includes(Permission.EGG_EDIT) ?? false
  );

  $effect(() => {
    data.eggs.then((d) => {
      eggs = d ?? [];
    });
  });

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
    const res = await deleteEgg({ body: { uuid: selected.uuid } });
    isLoading = false;

    if (res.error) {
      return { error: apiErrorMessage(res.error, 'Failed to delete egg') };
    }

    toast.success(`Egg ${selected.name} deleted successfully`);
    invalidate((url) => url.pathname.startsWith('/api/eggs'));
  };

  const onimport = (eggImport: EggImport) => {
    goto('/eggs/create', { state: { eggImport } });
  };

  const startDeleteEgg = (item: EggListItem) => {
    selected = item;
    deleteOpen = true;
  };
</script>

<div class="flex flex-col gap-4 p-4">
  <div class="ml-7 flex flex-wrap items-start gap-3 md:m-0">
    <div class="grid gap-1">
      <h3 class="text-xl font-medium">Eggs</h3>
      <p class="text-muted-foreground text-sm">
        Server templates that define how game servers are installed and run.
      </p>
    </div>
    <!-- The empty state carries the same actions; avoid duplicate controls. -->
    {#if eggs?.length !== 0}
      <div class="ml-auto flex gap-2">
        {@render actions()}
      </div>
    {/if}
  </div>
  <Separator />
  {#if eggs?.length === 0}
    <Empty.Root class="border border-dashed">
      <Empty.Header>
        <Empty.Media variant="icon"><Egg /></Empty.Media>
        <Empty.Title>No eggs yet</Empty.Title>
        <Empty.Description>
          Create an egg from scratch, or import a Pterodactyl or Pelican JSON
          file.
        </Empty.Description>
      </Empty.Header>
      <Empty.Content class="flex-row justify-center">
        {@render actions()}
      </Empty.Content>
    </Empty.Root>
  {:else}
    <div class="grid gap-2">
      <Table
        data={data.eggs}
        {columns}
        columnData={{ canEdit, deleteEgg: startDeleteEgg }}
        searchColumns={['name', 'author', 'description']}
      />
      {#if eggs}
        <span class="text-muted-foreground px-1 text-xs">
          {eggs.length}
          {eggs.length === 1 ? 'egg' : 'eggs'}
        </span>
      {/if}
    </div>
  {/if}
</div>

{#snippet actions()}
  <ImportButton {onimport} disabled={!canEdit} />
  <Button class="cursor-pointer" href="/eggs/create" disabled={!canEdit}>
    <Plus />
    Create egg
  </Button>
{/snippet}

<FormDialog
  title={`Delete egg ${selected?.name}?`}
  description="This cannot be undone. The egg and its variables, scripts and config files are removed."
  confirm="Delete"
  confirmVariant="destructive"
  onsubmit={deleteItemConfirm}
  bind:open={deleteOpen}
  bind:isLoading
  schema={z.object({})}
/>
