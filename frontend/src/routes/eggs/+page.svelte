<script lang="ts">
  import { Button } from '@profidev/pleiades/components/ui/button';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import Plus from '@lucide/svelte/icons/plus';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import { Spinner } from '@profidev/pleiades/components/ui/spinner';
  import Table from '@profidev/pleiades/components/table/clean-table.svelte';
  import { columns } from './table.svelte';
  import { z } from 'zod';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { goto, invalidate } from '$app/navigation';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
  import Egg from '@lucide/svelte/icons/egg';
  import ImportButton from './ImportButton.svelte';
  import { Permission } from '$lib/permissions.svelte';
  import { deleteEgg, type EggListItem, type UserInfo } from '$lib/client';
  import { deleteError, type EggImport } from './schema.svelte';
  import { checkEggUpdate, type EggUpdate } from './update';
  import UpdateDialog from './UpdateDialog.svelte';

  const { data } = $props();

  let selected: EggListItem | undefined = $state();
  let deleteOpen = $state(false);
  let isLoading = $state(false);
  let user: UserInfo | undefined = $state();
  // Undefined while loading; drives the count caption and the empty state.
  let eggs: EggListItem[] | undefined = $state();
  let checking = $state(false);
  // Last check result per egg uuid: available update or failure reason.
  let updates: Record<string, EggUpdate> = $state({});
  let failed: Record<string, string> = $state({});
  let reviewing: EggUpdate | undefined = $state.raw();
  let reviewOpen = $state(false);

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

    if (!res.response?.ok) {
      return { error: deleteError(res.response?.status) };
    }

    toast.success(`Egg ${selected.name} deleted successfully`);
    invalidate((url) => url.pathname.startsWith('/api/eggs'));
  };

  const onimport = (eggImport: EggImport) => {
    goto('/eggs/create', { state: { eggImport } });
  };

  const plural = (n: number, word: string) =>
    `${n} ${word}${n === 1 ? '' : 's'}`;

  const review = (update: EggUpdate) => {
    reviewing = update;
    reviewOpen = true;
  };

  const checkAll = async () => {
    const targets = (eggs ?? []).filter((e) => e.has_update_url);
    if (!targets.length) {
      toast.info('No egg has an update URL', {
        description: 'Add an Update URL in General to check an egg for updates.'
      });
      return;
    }

    checking = true;
    const results = await Promise.allSettled(
      targets.map((e) => checkEggUpdate(e.uuid))
    );
    checking = false;

    updates = {};
    failed = {};
    results.forEach((res, i) => {
      const { uuid } = targets[i];
      if (res.status === 'rejected') {
        failed[uuid] =
          res.reason instanceof Error ? res.reason.message : 'Check failed';
      } else if (res.value.changed.length) {
        updates[uuid] = res.value;
      }
    });

    const found = Object.values(updates);
    const failedEggs = targets.filter((e) => failed[e.uuid]);
    if (failedEggs.length) {
      toast.error(`Couldn't check ${plural(failedEggs.length, 'egg')}`, {
        description: failedEggs
          .map((e) => `${e.name} (${failed[e.uuid]})`)
          .join(', '),
        action: { label: 'Retry', onClick: checkAll }
      });
    }
    if (found.length) {
      toast.info(
        found.length === 1
          ? '1 update available'
          : `${found.length} updates available`,
        {
          description: found.map((u) => u.current.name).join(', '),
          action: { label: 'Review', onClick: () => review(found[0]) }
        }
      );
    } else if (!failedEggs.length) {
      toast.success('All eggs are up to date', {
        description: 'Every egg matches its update URL.'
      });
    }
  };

  const onapplied = () => {
    if (reviewing) delete updates[reviewing.uuid];
    invalidate((url) => url.pathname.startsWith('/api/eggs'));
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
      <div class="ml-auto flex flex-wrap gap-2">
        <Button
          variant="outline"
          class="cursor-pointer"
          onclick={checkAll}
          disabled={checking || !canEdit || !eggs}
        >
          {#if checking}
            <Spinner />
            Checking…
          {:else}
            <RefreshCw />
            Check for updates
          {/if}
        </Button>
        {@render actions()}
      </div>
    {/if}
  </div>
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
        columnData={{
          canEdit,
          deleteEgg: startDeleteEgg,
          updates,
          failed,
          review
        }}
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
    Create
  </Button>
{/snippet}

<UpdateDialog bind:open={reviewOpen} update={reviewing} {onapplied} />

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
