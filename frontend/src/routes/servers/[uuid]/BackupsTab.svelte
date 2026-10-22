<script lang="ts">
  import Archive from '@lucide/svelte/icons/archive';
  import Download from '@lucide/svelte/icons/download';
  import History from '@lucide/svelte/icons/history';
  import Info from '@lucide/svelte/icons/info';
  import Lock from '@lucide/svelte/icons/lock';
  import LockOpen from '@lucide/svelte/icons/lock-open';
  import Plus from '@lucide/svelte/icons/plus';
  import Trash from '@lucide/svelte/icons/trash-2';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormSwitch from '@profidev/pleiades/components/form/form-switch.svelte';
  import FormTextarea from '@profidev/pleiades/components/form/form-textarea.svelte';
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import * as AlertDialog from '@profidev/pleiades/components/ui/alert-dialog';
  import { Badge } from '@profidev/pleiades/components/ui/badge';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Checkbox } from '@profidev/pleiades/components/ui/checkbox';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import { Progress } from '@profidev/pleiades/components/ui/progress';
  import { Skeleton } from '@profidev/pleiades/components/ui/skeleton';
  import { Spinner } from '@profidev/pleiades/components/ui/spinner';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { untrack } from 'svelte';
  import {
    createBackup,
    deleteBackup,
    listBackups,
    lockBackup,
    restoreBackup,
    type BackupInfo,
    type BackupList,
    type ServerInfo
  } from '$lib/client';
  import { formatBytes } from '../format';
  import { relativeTime } from './files';
  import {
    applyBackupFrame,
    backupProgress,
    createBackupSchema,
    defaultBackupName,
    IGNORE_PLACEHOLDER,
    isBusy,
    pruneProgress,
    restoreFailed,
    shortHash,
    withProgress,
    type Progress as LiveProgress
  } from './backups';
  import type { ServerLive } from './live.svelte';

  interface Props {
    server: ServerInfo;
    live: ServerLive;
  }

  let { server, live }: Props = $props();

  let uuid = $derived(server.uuid);
  let canWrite = $derived(
    server.access === 'admin' ||
      (server.access === 'write' && !server.suspended)
  );

  /** Undefined while loading. */
  let list: BackupList | undefined = $state.raw();
  let failed = $state(false);
  let progress: LiveProgress = $state.raw({});

  let backups = $derived(
    (list?.backups ?? []).map((b) => withProgress(b, progress))
  );
  let atLimit = $derived(!!list && list.used >= list.limit);

  let createOpen = $state(false);
  let createLoading = $state(false);
  let restoring: BackupInfo | undefined = $state.raw();
  let restoreOpen = $state(false);
  let truncate = $state(false);
  let deleting: BackupInfo | undefined = $state.raw();
  let deleteOpen = $state(false);
  let pending = $state(false);

  let loadSeq = 0;
  const load = async () => {
    const seq = ++loadSeq;
    const res = await listBackups({ path: { uuid } });
    if (seq !== loadSeq) return;
    failed = !res.data;
    if (res.data) {
      list = res.data;
      progress = pruneProgress(progress, res.data.backups);
    }
  };

  $effect(() => {
    // Refetch per server only, not on every status change of `server`.
    void uuid;
    list = undefined;
    progress = {};
    load();
  });

  // frames missed while disconnected are caught up by a reload
  $effect(() => {
    if (live.connected) untrack(load);
  });

  $effect(() =>
    live.on('backup', (frame) => {
      if (restoreFailed(progress, frame)) {
        const name = list?.backups.find((b) => b.id === frame.id)?.name;
        toast.error(`Couldn't restore ${name ?? 'the backup'}`);
      }
      const next = applyBackupFrame(progress, frame);
      progress = next.progress;
      if (next.reload) load();
    })
  );

  const retry = () => {
    failed = false;
    list = undefined;
    load();
  };

  const progressOf = (b: ReturnType<typeof withProgress>) =>
    b.status === 'restoring'
      ? backupProgress(b.bytes, b.size_bytes, 'restored')
      : backupProgress(b.bytes, live.stats?.disk_bytes, 'archived');

  const submitCreate = async (form: {
    name: string;
    ignored: string;
    locked: boolean;
  }) => {
    const res = await createBackup({
      body: {
        ignored: form.ignored.trim() ? form.ignored : null,
        locked: form.locked,
        name: form.name || null
      },
      path: { uuid }
    });
    if (!res.data) {
      load();
      return {
        error:
          res.response?.status === 409
            ? 'Backup limit reached'
            : "Couldn't start the backup"
      };
    }
    toast.success(`Backup ${res.data.name} started`);
    load();
  };

  const toggleLock = async (b: BackupInfo) => {
    const res = await lockBackup({
      body: { locked: !b.locked },
      path: { backup: b.id, uuid }
    });
    if (!res.response?.ok) {
      toast.error(`Couldn't ${b.locked ? 'unlock' : 'lock'} ${b.name}`);
      return;
    }
    toast.success(`${b.locked ? 'Unlocked' : 'Locked'} ${b.name}`);
    load();
  };

  const askRestore = (b: BackupInfo) => {
    restoring = b;
    truncate = false;
    restoreOpen = true;
  };

  const confirmRestore = async () => {
    if (!restoring) return;
    pending = true;
    const res = await restoreBackup({
      body: { truncate },
      path: { backup: restoring.id, uuid }
    });
    pending = false;
    if (!res.response?.ok) {
      toast.error(
        res.response?.status === 409
          ? 'Only completed backups can be restored, and not while another operation runs'
          : `Couldn't restore ${restoring.name}`
      );
      return;
    }
    restoreOpen = false;
    toast.success(`Restoring ${restoring.name}`);
    load();
  };

  const askDelete = (b: BackupInfo) => {
    deleting = b;
    deleteOpen = true;
  };

  const confirmDelete = async () => {
    if (!deleting) return;
    pending = true;
    const res = await deleteBackup({ path: { backup: deleting.id, uuid } });
    pending = false;
    if (!res.response?.ok) {
      toast.error(
        res.response?.status === 409
          ? `${deleting.name} is locked or in use`
          : `Couldn't delete ${deleting.name}`
      );
      return;
    }
    deleteOpen = false;
    toast.success(`Deleted ${deleting.name}`);
    load();
  };
</script>

<div class="grid gap-3">
  <div class="flex flex-wrap items-end gap-x-6 gap-y-3">
    <div class="mr-auto">
      <h2 class="text-lg font-semibold">Backups</h2>
      <p class="text-muted-foreground text-sm">
        Snapshots of the server files. Restore one to roll back.
      </p>
    </div>
    {#if list}
      <div class="grid w-40 gap-1.5">
        <div
          class="text-muted-foreground flex justify-between text-xs"
          id="backups-used"
        >
          <span>Backups used</span>
          <span class="font-mono">{list.used} of {list.limit}</span>
        </div>
        <Progress
          value={list.limit ? (list.used / list.limit) * 100 : 0}
          class="h-1"
          aria-labelledby="backups-used"
        />
      </div>
    {/if}
    {#if canWrite}
      <Button
        class="cursor-pointer"
        disabled={!list || atLimit}
        onclick={() => (createOpen = true)}
      >
        <Plus />
        Create backup
      </Button>
    {/if}
  </div>

  {#if list && atLimit && canWrite}
    <Alert.Root>
      <Info />
      <Alert.Title>Backup limit reached</Alert.Title>
      <Alert.Description>
        This server can keep {list.limit}
        {list.limit === 1 ? 'backup' : 'backups'}. Delete an unlocked backup to
        create a new one. Locked backups can't be deleted until you unlock them.
      </Alert.Description>
    </Alert.Root>
  {/if}

  {#if failed}
    <Alert.Root variant="destructive">
      <Alert.Title>Failed to load backups</Alert.Title>
      <Alert.Description>
        The panel could not list the backups of this server.
        <Button
          variant="outline"
          size="sm"
          class="mt-2 cursor-pointer"
          onclick={retry}
        >
          Retry
        </Button>
      </Alert.Description>
    </Alert.Root>
  {:else if !list}
    <div class="grid gap-3" aria-label="Loading backups">
      {#each [1, 2, 3] as i (i)}
        <div class="flex items-center gap-3 rounded-lg border p-4">
          <Skeleton class="size-10 rounded-lg" />
          <div class="grid grow gap-2">
            <Skeleton class="h-4 w-1/3" />
            <Skeleton class="h-3 w-1/2" />
          </div>
        </div>
      {/each}
    </div>
  {:else if backups.length === 0}
    <Empty.Root class="border border-dashed">
      <Empty.Header>
        <Empty.Media variant="icon"><Archive /></Empty.Media>
        <Empty.Title>No backups yet</Empty.Title>
        <Empty.Description>
          A backup is a snapshot of the server files you can restore later.
        </Empty.Description>
      </Empty.Header>
      {#if canWrite}
        <Empty.Content>
          <Button class="cursor-pointer" onclick={() => (createOpen = true)}>
            <Plus />
            Create backup
          </Button>
        </Empty.Content>
      {/if}
    </Empty.Root>
  {:else}
    <ul class="grid gap-2" aria-label="Backups">
      {#each backups as b (b.id)}
        {@const busy = isBusy(b.status)}
        <li
          class="flex flex-col gap-3 rounded-lg border p-4 sm:flex-row sm:items-center"
        >
          <div class="flex min-w-0 grow items-start gap-3 sm:items-center">
            <div
              class="bg-muted text-muted-foreground flex size-10 flex-none items-center justify-center rounded-lg"
            >
              <Archive class="size-5" />
            </div>
            <div class="grid min-w-0 grow gap-1">
              <div class="flex flex-wrap items-center gap-2">
                <span class="truncate font-medium">{b.name}</span>
                {#if b.locked}
                  <Badge variant="outline"><Lock />Locked</Badge>
                {/if}
                {#if b.status === 'completed'}
                  <Badge variant="secondary">Completed</Badge>
                {:else if b.status === 'failed'}
                  <Badge variant="destructive" title={b.error ?? undefined}
                    >Failed</Badge
                  >
                {:else}
                  <Badge variant="outline">
                    <Spinner />
                    {b.status === 'creating' ? 'Creating' : 'Restoring'}
                  </Badge>
                {/if}
              </div>
              {#if busy}
                {@const p = progressOf(b)}
                <Progress
                  value={p.percent ?? null}
                  class="h-1.5 max-w-md"
                  aria-label={`Progress of ${b.name}`}
                />
                <span class="text-muted-foreground text-xs">{p.text}</span>
              {:else}
                <div
                  class="text-muted-foreground flex flex-wrap gap-x-3 font-mono text-xs"
                >
                  <span>{formatBytes(b.size_bytes)}</span>
                  <span class="font-sans" title={b.created_at.toLocaleString()}
                    >{relativeTime(b.created_at)}</span
                  >
                  {#if b.sha256}
                    <span title={`sha256 ${b.sha256}`}
                      >{shortHash(b.sha256)}</span
                    >
                  {/if}
                  {#if b.status === 'failed' && b.error}
                    <span class="text-destructive font-sans">{b.error}</span>
                  {/if}
                </div>
              {/if}
            </div>
          </div>
          <div class="flex flex-none items-center gap-1 pl-13 sm:pl-0">
            {#if b.status === 'completed' && !busy}
              <Button
                variant="ghost"
                size="icon-sm"
                href={`/api/servers/${uuid}/backups/${b.id}/download`}
                download
                data-sveltekit-reload
                aria-label={`Download ${b.name}`}
              >
                <Download />
              </Button>
            {:else}
              <Button
                variant="ghost"
                size="icon-sm"
                disabled
                aria-label={`Download ${b.name}`}
              >
                <Download />
              </Button>
            {/if}
            {#if canWrite}
              <Button
                variant="ghost"
                size="icon-sm"
                class="cursor-pointer"
                disabled={busy || b.status !== 'completed'}
                aria-label={`Restore ${b.name}`}
                onclick={() => askRestore(b)}
              >
                <History />
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                class="cursor-pointer"
                disabled={busy}
                aria-label={`${b.locked ? 'Unlock' : 'Lock'} ${b.name}`}
                onclick={() => toggleLock(b)}
              >
                {#if b.locked}<Lock />{:else}<LockOpen />{/if}
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                class="text-destructive hover:text-destructive cursor-pointer"
                disabled={busy || b.locked}
                aria-label={`Delete ${b.name}`}
                onclick={() => askDelete(b)}
              >
                <Trash />
              </Button>
            {/if}
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

{#if canWrite}
  <FormDialog
    title="Create backup"
    description="The server keeps running. Large worlds can take a few minutes."
    confirm="Start backup"
    schema={createBackupSchema}
    onsubmit={submitCreate}
    bind:open={createOpen}
    bind:isLoading={createLoading}
  >
    {#snippet children({ props })}
      <div class="grid gap-1">
        <FormInput
          {...props}
          key="name"
          label="Name"
          placeholder={defaultBackupName()}
          autocomplete="off"
        />
        <p class="text-muted-foreground text-xs">
          Optional. A timestamp is used when empty.
        </p>
      </div>
      <div class="grid gap-1">
        <FormTextarea
          {...props}
          key="ignored"
          label="Ignored files"
          rows={3}
          class="font-mono"
          placeholder={IGNORE_PLACEHOLDER}
        />
        <p class="text-muted-foreground text-xs">
          One pattern per line, .gitignore style. The server's .smaugignore,
          .pteroignore and .pelicanignore also apply.
        </p>
      </div>
      <div class="grid gap-0.5">
        <FormSwitch {...props} key="locked" label="Lock this backup" />
        <p class="text-muted-foreground text-xs">
          Locked backups can't be deleted by mistake.
        </p>
      </div>
    {/snippet}
  </FormDialog>
{/if}

<AlertDialog.Root bind:open={restoreOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Restore this backup?</AlertDialog.Title>
      <AlertDialog.Description>
        The server is stopped, then the files from "{restoring?.name}" are
        written over the current ones. Changes made after this backup are lost.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <div class="flex items-start gap-2">
      <Checkbox id="restore-truncate" bind:checked={truncate} />
      <div class="grid gap-0.5">
        <Label for="restore-truncate">Delete all current files first</Label>
        <p class="text-muted-foreground text-xs">
          Removes files that aren't in the backup. Otherwise they stay.
        </p>
      </div>
    </div>
    <AlertDialog.Footer>
      <AlertDialog.Cancel class="cursor-pointer">Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        variant="destructive"
        class="cursor-pointer"
        disabled={pending}
        onclick={confirmRestore}
      >
        Restore backup
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>

<AlertDialog.Root bind:open={deleteOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Delete this backup?</AlertDialog.Title>
      <AlertDialog.Description>
        "{deleting?.name}" ({formatBytes(deleting?.size_bytes ?? 0)}) is removed
        for good. This cannot be undone.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel class="cursor-pointer">Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        variant="destructive"
        class="cursor-pointer"
        disabled={pending}
        onclick={confirmDelete}
      >
        Delete backup
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
