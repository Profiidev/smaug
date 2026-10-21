<script lang="ts">
  import { Button } from '@profidev/pleiades/components/ui/button';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import Plus from '@lucide/svelte/icons/plus';
  import Server from '@lucide/svelte/icons/server';
  import Table from '@profidev/pleiades/components/table/clean-table.svelte';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
  import * as NativeSelect from '@profidev/pleiades/components/ui/native-select';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { invalidate } from '$app/navigation';
  import { z } from 'zod';
  import { Permission } from '$lib/permissions.svelte';
  import { ServerStatus, deleteServer, type UserInfo } from '$lib/client';
  import { columns, toRow, type ServerRow } from './table.svelte';
  import { STATUS } from './status';

  const { data } = $props();

  const ERRORS: Record<string, string> = {
    not_found: 'Server not found',
    options: 'Failed to load server options',
    other: 'Failed to load server'
  };

  let selected: ServerRow | undefined = $state();
  let deleteOpen = $state(false);
  let isLoading = $state(false);
  let user: UserInfo | undefined = $state();
  // Undefined while loading; drives the table skeleton, count and empty state.
  let servers: ServerRow[] | undefined = $state();
  let status = $state('');

  let canEdit = $derived(
    user?.permissions.includes(Permission.SERVER_EDIT) ?? false
  );
  let filtered = $derived(
    status ? servers?.filter((s) => s.status === status) : servers
  );

  $effect(() => {
    data.servers.then((d) => {
      servers = (d ?? []).map(toRow);
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
    const res = await deleteServer({ path: { uuid: selected.uuid } });
    isLoading = false;

    if (!res.response?.ok) {
      return { error: 'Failed to delete server' };
    }

    toast.success(`Server ${selected.name} deleted successfully`);
    invalidate((url) => url.pathname.startsWith('/api/servers'));
  };

  const startDelete = (item: ServerRow) => {
    selected = item;
    deleteOpen = true;
  };
</script>

<div class="flex flex-col gap-4 p-4">
  <div class="ml-7 flex flex-wrap items-start gap-3 md:m-0">
    <div class="grid gap-1">
      <h3 class="text-xl font-medium">Servers</h3>
      <p class="text-muted-foreground text-sm">
        Game servers running on your nodes.
      </p>
    </div>
    <!-- The empty state carries the same action; avoid duplicate controls. -->
    {#if servers?.length !== 0}
      <div class="ml-auto flex flex-wrap gap-2">
        <NativeSelect.Root
          aria-label="Filter by status"
          bind:value={status}
          disabled={!servers}
        >
          <NativeSelect.Option value="">All statuses</NativeSelect.Option>
          {#each Object.values(ServerStatus) as value (value)}
            <NativeSelect.Option {value}
              >{STATUS[value].label}</NativeSelect.Option
            >
          {/each}
        </NativeSelect.Root>
        {@render create()}
      </div>
    {/if}
  </div>
  <Separator />
  {#if servers?.length === 0}
    <Empty.Root class="border border-dashed">
      <Empty.Header>
        <Empty.Media variant="icon"><Server /></Empty.Media>
        <Empty.Title>No servers yet</Empty.Title>
        <Empty.Description>
          Create a server from an egg on one of your nodes.
        </Empty.Description>
      </Empty.Header>
      <Empty.Content class="flex-row justify-center">
        {@render create()}
      </Empty.Content>
    </Empty.Root>
  {:else}
    <div class="grid gap-2">
      <Table
        data={filtered}
        {columns}
        columnData={{ canEdit, deleteServer: startDelete }}
        searchColumns={['name', 'node_name', 'allocation']}
      />
      {#if filtered}
        <span class="text-muted-foreground px-1 text-xs">
          {filtered.length}
          {filtered.length === 1 ? 'server' : 'servers'}
        </span>
      {/if}
    </div>
  {/if}
</div>

{#snippet create()}
  <Button class="cursor-pointer" href="/servers/create" disabled={!canEdit}>
    <Plus />
    Create server
  </Button>
{/snippet}

<FormDialog
  title={`Delete server ${selected?.name}?`}
  description="This cannot be undone. The server's files are removed and its allocations freed."
  confirm="Delete"
  confirmVariant="destructive"
  onsubmit={deleteItemConfirm}
  bind:open={deleteOpen}
  bind:isLoading
  schema={z.object({})}
/>
