<script lang="ts">
  import CalendarClock from '@lucide/svelte/icons/calendar-clock';
  import Pencil from '@lucide/svelte/icons/pencil';
  import Play from '@lucide/svelte/icons/play';
  import Plus from '@lucide/svelte/icons/plus';
  import Trash from '@lucide/svelte/icons/trash-2';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormSwitch from '@profidev/pleiades/components/form/form-switch.svelte';
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import * as AlertDialog from '@profidev/pleiades/components/ui/alert-dialog';
  import { Badge } from '@profidev/pleiades/components/ui/badge';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
  import { Skeleton } from '@profidev/pleiades/components/ui/skeleton';
  import { Spinner } from '@profidev/pleiades/components/ui/spinner';
  import * as Table from '@profidev/pleiades/components/ui/table';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { untrack } from 'svelte';
  import {
    createSchedule,
    deleteSchedule,
    listSchedules,
    runSchedule,
    updateSchedule,
    type ScheduleInfo,
    type ServerInfo
  } from '$lib/client';
  import type { ServerLive } from './live.svelte';
  import ScheduleDetail from './ScheduleDetail.svelte';
  import {
    applyScheduleFrame,
    CRON_FIELDS,
    cronString,
    cronText,
    fieldError,
    lastRunText,
    nextRunText,
    scheduleBody,
    scheduleForm,
    scheduleSchema,
    STATUS_LABELS,
    pruneRuns,
    withRun,
    type Runs,
    type ScheduleForm
  } from './schedules';

  interface Props {
    server: ServerInfo;
    live: ServerLive;
  }

  let { server, live }: Props = $props();

  const FORM_FIELDS = [
    'name',
    'minute',
    'hour',
    'day_of_month',
    'month',
    'day_of_week'
  ] as const;

  let uuid = $derived(server.uuid);
  let canWrite = $derived(
    server.access === 'admin' ||
      (server.access === 'write' && !server.suspended)
  );
  let selectedId = $derived(page.url.searchParams.get('id'));

  /** Undefined while loading. */
  let list: ScheduleInfo[] | undefined = $state.raw();
  let failed = $state(false);
  let runs: Runs = $state.raw({});

  let schedules = $derived((list ?? []).map((s) => withRun(s, runs)));
  let selected = $derived(schedules.find((s) => s.id === selectedId));

  /** `undefined` creates, a schedule edits. */
  let editing: ScheduleInfo | undefined = $state.raw();
  let formOpen = $state(false);
  let formLoading = $state(false);
  let deleting: ScheduleInfo | undefined = $state.raw();
  let deleteOpen = $state(false);
  let pending = $state(false);

  let loadSeq = 0;
  const load = async () => {
    const seq = ++loadSeq;
    const res = await listSchedules({ path: { uuid } });
    if (seq !== loadSeq) return;
    failed = !res.data;
    if (res.data) {
      list = res.data;
      runs = pruneRuns(runs, res.data);
    }
  };

  $effect(() => {
    void uuid;
    list = undefined;
    runs = {};
    load();
  });

  // frames missed while disconnected are caught up by a reload
  $effect(() => {
    if (live.connected) untrack(load);
  });

  $effect(() =>
    live.on('schedule', (frame) => {
      const next = applyScheduleFrame(runs, frame);
      runs = next.runs;
      if (next.reload) load();
    })
  );

  // A deep link to a schedule that does not exist goes back to the list.
  $effect(() => {
    if (list && selectedId && !list.some((s) => s.id === selectedId)) {
      toast.error('Schedule not found');
      open(undefined);
    }
  });

  const hrefFor = (id?: string) =>
    id
      ? `${page.url.pathname}?id=${encodeURIComponent(id)}`
      : page.url.pathname;

  const open = (id: string | undefined) =>
    goto(hrefFor(id), { keepFocus: true, noScroll: true });

  const retry = () => {
    failed = false;
    list = undefined;
    load();
  };

  const openForm = (s?: ScheduleInfo) => {
    editing = s;
    formOpen = true;
  };

  const submitForm = async (form: ScheduleForm) => {
    const body = scheduleBody(form);
    const res = editing
      ? await updateSchedule({
          body,
          path: { schedule: editing.id, uuid }
        })
      : await createSchedule({ body, path: { uuid } });
    if (!res.data) {
      return (
        fieldError(res.error, FORM_FIELDS) ?? {
          error: "Couldn't save the schedule"
        }
      );
    }
    toast.success(
      editing ? `Saved ${res.data.name}` : `Created ${res.data.name}`
    );
    await load();
    if (!editing) open(res.data.id);
  };

  const run = async (s: ScheduleInfo) => {
    const res = await runSchedule({ path: { schedule: s.id, uuid } });
    if (res.response?.status === 409) {
      toast.error(
        s.only_when_online && server.status !== 'running'
          ? `${s.name} only runs while the server is online. Start the server first.`
          : `${s.name} is already running. Wait until it finishes.`
      );
      return;
    }
    if (!res.response?.ok) {
      toast.error(`Couldn't run ${s.name}`);
      return;
    }
    toast.success(`Running ${s.name}`);
    load();
  };

  /** PATCH of one switch from the detail view. */
  const patch = async (
    s: ScheduleInfo,
    body: { enabled?: boolean; only_when_online?: boolean }
  ) => {
    const res = await updateSchedule({
      body,
      path: { schedule: s.id, uuid }
    });
    if (!res.data) toast.error(`Couldn't update ${s.name}`);
    await load();
  };

  const askDelete = (s: ScheduleInfo) => {
    deleting = s;
    deleteOpen = true;
  };

  const confirmDelete = async () => {
    if (!deleting) return;
    pending = true;
    const res = await deleteSchedule({
      path: { schedule: deleting.id, uuid }
    });
    pending = false;
    if (!res.response?.ok) {
      toast.error(`Couldn't delete ${deleting.name}`);
      return;
    }
    deleteOpen = false;
    toast.success(`Deleted ${deleting.name}`);
    if (selectedId === deleting.id) await open(undefined);
    load();
  };
</script>

{#snippet statusBadge(s: ScheduleInfo)}
  {#if s.status === 'running'}
    <Badge variant="outline"><Spinner />Running</Badge>
  {:else}
    <Badge variant={s.status === 'active' ? 'secondary' : 'outline'}
      >{STATUS_LABELS[s.status]}</Badge
    >
  {/if}
{/snippet}

{#snippet rowActions(s: ScheduleInfo)}
  {#if canWrite}
    <div class="flex items-center justify-end gap-1">
      <Button
        variant="ghost"
        size="icon-sm"
        class="cursor-pointer"
        disabled={s.status === 'running'}
        aria-label={`Run ${s.name} now`}
        onclick={() => run(s)}
      >
        <Play />
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        class="cursor-pointer"
        aria-label={`Edit ${s.name}`}
        onclick={() => openForm(s)}
      >
        <Pencil />
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        class="text-destructive hover:text-destructive cursor-pointer"
        aria-label={`Delete ${s.name}`}
        onclick={() => askDelete(s)}
      >
        <Trash />
      </Button>
    </div>
  {/if}
{/snippet}

{#if selectedId && selected}
  <ScheduleDetail
    schedule={selected}
    {server}
    {canWrite}
    backHref={hrefFor()}
    {statusBadge}
    onrun={run}
    onedit={openForm}
    ondelete={askDelete}
    onpatch={patch}
    onchange={load}
  />
{:else}
  <div class="grid gap-3">
    <div class="flex flex-wrap items-end gap-3">
      <div class="mr-auto">
        <h2 class="text-lg font-semibold">Schedules</h2>
        <p class="text-muted-foreground text-sm">
          Run commands, power actions and backups on a timer.
        </p>
      </div>
      {#if canWrite}
        <Button class="cursor-pointer" onclick={() => openForm()}>
          <Plus />
          Create schedule
        </Button>
      {/if}
    </div>

    {#if failed}
      <Alert.Root variant="destructive">
        <Alert.Title>Failed to load schedules</Alert.Title>
        <Alert.Description>
          The panel could not list the schedules of this server.
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
    {:else if !list || (selectedId && !selected)}
      <div
        class="grid gap-3 rounded-md border p-4"
        aria-label="Loading schedules"
      >
        {#each [1, 2, 3] as i (i)}
          <div class="flex items-center gap-3">
            <Skeleton class="h-4 w-1/4" />
            <Skeleton class="h-4 grow" />
            <Skeleton class="h-4 w-16" />
          </div>
        {/each}
      </div>
    {:else if schedules.length === 0}
      <Empty.Root class="border border-dashed">
        <Empty.Header>
          <Empty.Media variant="icon"><CalendarClock /></Empty.Media>
          <Empty.Title>No schedules yet</Empty.Title>
          <Empty.Description>
            Restart the server every night, save the world hourly or take weekly
            backups.
          </Empty.Description>
        </Empty.Header>
        {#if canWrite}
          <Empty.Content>
            <Button class="cursor-pointer" onclick={() => openForm()}>
              <Plus />
              Create schedule
            </Button>
          </Empty.Content>
        {/if}
      </Empty.Root>
    {:else}
      <div class="hidden rounded-md border md:block">
        <Table.Root>
          <Table.Header>
            <Table.Row>
              <Table.Head>Name</Table.Head>
              <Table.Head>Cron</Table.Head>
              <Table.Head>Last run</Table.Head>
              <Table.Head>Next run</Table.Head>
              <Table.Head>Tasks</Table.Head>
              <Table.Head>Status</Table.Head>
              {#if canWrite}<Table.Head class="text-right">Actions</Table.Head
                >{/if}
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {#each schedules as s (s.id)}
              <Table.Row>
                <Table.Cell>
                  <a class="font-medium hover:underline" href={hrefFor(s.id)}
                    >{s.name}</a
                  >
                </Table.Cell>
                <Table.Cell>
                  <div class="font-mono">{cronString(s.cron)}</div>
                  <div class="text-muted-foreground text-xs">
                    {cronText(s.cron)}
                  </div>
                </Table.Cell>
                <Table.Cell
                  class="text-muted-foreground"
                  title={s.last_run_at?.toLocaleString()}
                  >{lastRunText(s)}</Table.Cell
                >
                <Table.Cell
                  class="text-muted-foreground"
                  title={s.next_run_at?.toLocaleString()}
                  >{nextRunText(s)}</Table.Cell
                >
                <Table.Cell class="text-muted-foreground"
                  >{s.tasks.length}</Table.Cell
                >
                <Table.Cell>{@render statusBadge(s)}</Table.Cell>
                {#if canWrite}<Table.Cell>{@render rowActions(s)}</Table.Cell
                  >{/if}
              </Table.Row>
            {/each}
          </Table.Body>
        </Table.Root>
      </div>
      <ul class="grid gap-2 md:hidden" aria-label="Schedules">
        {#each schedules as s (s.id)}
          <li class="grid gap-2 rounded-lg border p-4">
            <div class="flex items-center gap-2">
              <a
                class="min-w-0 grow truncate font-medium hover:underline"
                href={hrefFor(s.id)}>{s.name}</a
              >
              {@render statusBadge(s)}
            </div>
            <div>
              <div class="font-mono text-sm">{cronString(s.cron)}</div>
              <div class="text-muted-foreground text-xs">
                {cronText(s.cron)}
              </div>
            </div>
            <div class="text-muted-foreground text-xs">
              Last {lastRunText(s)} · Next {nextRunText(s)} · {s.tasks.length}
              {s.tasks.length === 1 ? 'task' : 'tasks'}
            </div>
            {@render rowActions(s)}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}

{#if canWrite && formOpen}
  <FormDialog
    title={editing ? 'Edit schedule' : 'Create schedule'}
    description="Times use the node's timezone (UTC)."
    confirm={editing ? 'Save schedule' : 'Create schedule'}
    schema={scheduleSchema}
    initialValue={scheduleForm(editing)}
    onsubmit={submitForm}
    bind:open={formOpen}
    bind:isLoading={formLoading}
  >
    {#snippet children({ props })}
      <FormInput {...props} key="name" label="Name" autocomplete="off" />
      <fieldset class="grid gap-1">
        <legend class="mb-1 text-sm font-medium">Cron</legend>
        <div class="grid grid-cols-3 gap-2 sm:grid-cols-5">
          {#each CRON_FIELDS as [key, label] (key)}
            <FormInput
              {...props}
              {key}
              {label}
              class="font-mono"
              autocomplete="off"
            />
          {/each}
        </div>
        <p class="text-muted-foreground text-xs">
          Use * for every value, */5 for every 5th, 1,15 for a list.
        </p>
      </fieldset>
      <div class="grid gap-0.5">
        <FormSwitch {...props} key="enabled" label="Enabled" />
        <p class="text-muted-foreground text-xs">
          Start running on the next match.
        </p>
      </div>
      <div class="grid gap-0.5">
        <FormSwitch
          {...props}
          key="only_when_online"
          label="Only when the server is online"
        />
        <p class="text-muted-foreground text-xs">
          Skips the run if the server is offline.
        </p>
      </div>
    {/snippet}
  </FormDialog>
{/if}

<AlertDialog.Root bind:open={deleteOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Delete {deleting?.name}?</AlertDialog.Title>
      <AlertDialog.Description>
        The schedule and its {deleting?.tasks.length ?? 0}
        {deleting?.tasks.length === 1 ? 'task are' : 'tasks are'} removed. This cannot
        be undone.
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
        Delete schedule
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
