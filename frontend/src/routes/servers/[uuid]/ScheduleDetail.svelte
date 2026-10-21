<script lang="ts">
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import Info from '@lucide/svelte/icons/info';
  import ListTodo from '@lucide/svelte/icons/list-todo';
  import Pencil from '@lucide/svelte/icons/pencil';
  import Play from '@lucide/svelte/icons/play';
  import Plus from '@lucide/svelte/icons/plus';
  import Trash from '@lucide/svelte/icons/trash-2';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import * as AlertDialog from '@profidev/pleiades/components/ui/alert-dialog';
  import { Badge } from '@profidev/pleiades/components/ui/badge';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Card from '@profidev/pleiades/components/ui/card';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { Spinner } from '@profidev/pleiades/components/ui/spinner';
  import { Switch } from '@profidev/pleiades/components/ui/switch';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { cn } from '@profidev/pleiades/utils';
  import type { Snippet } from 'svelte';
  import {
    createScheduleTask,
    deleteScheduleTask,
    updateScheduleTask,
    type ScheduleInfo,
    type ServerInfo,
    type TaskInfo
  } from '$lib/client';
  import {
    ACTION_LABELS,
    CRON_FIELDS,
    cronText,
    fieldError,
    lastRunText,
    nextRunText,
    offsetText,
    taskSchema,
    type TaskForm
  } from './schedules';
  import TaskFields from './TaskFields.svelte';

  interface Props {
    schedule: ScheduleInfo;
    server: ServerInfo;
    canWrite: boolean;
    backHref: string;
    statusBadge: Snippet<[ScheduleInfo]>;
    onrun: (s: ScheduleInfo) => void;
    onedit: (s: ScheduleInfo) => void;
    ondelete: (s: ScheduleInfo) => void;
    onpatch: (
      s: ScheduleInfo,
      body: { enabled?: boolean; only_when_online?: boolean }
    ) => Promise<void>;
    /** Tasks changed; reload the schedule. */
    onchange: () => Promise<void>;
  }

  let {
    schedule,
    server,
    canWrite,
    backHref,
    statusBadge,
    onrun,
    onedit,
    ondelete,
    onpatch,
    onchange
  }: Props = $props();

  const TASK_FIELDS = ['payload', 'offset_seconds'] as const;

  let uuid = $derived(server.uuid);
  let running = $derived(schedule.status === 'running');
  let tasks = $derived(
    schedule.tasks.toSorted((a, b) => a.position - b.position)
  );
  let active = $derived(running ? (schedule.running_task ?? null) : null);
  let activeTask = $derived(active === null ? undefined : tasks[active]);

  /** `undefined` adds, a task edits. */
  let editing: TaskInfo | undefined = $state.raw();
  let taskOpen = $state(false);
  let taskLoading = $state(false);
  let deleting: TaskInfo | undefined = $state.raw();
  let deleteOpen = $state(false);
  let pending = $state(false);
  let patching = $state(false);

  const openTask = (t?: TaskInfo) => {
    editing = t;
    taskOpen = true;
  };

  const submitTask = async (form: TaskForm) => {
    const body = {
      ...form,
      payload: form.action === 'power' ? form.payload.trim() : form.payload
    };
    const path = { schedule: schedule.id, uuid };
    const res = editing
      ? await updateScheduleTask({
          body,
          path: { ...path, task: editing.id }
        })
      : await createScheduleTask({ body, path });
    if (!res.data) {
      return (
        fieldError(res.error, TASK_FIELDS) ?? {
          error: "Couldn't save the task"
        }
      );
    }
    toast.success(editing ? 'Task saved' : 'Task added');
    await onchange();
  };

  const confirmDelete = async () => {
    if (!deleting) return;
    pending = true;
    const res = await deleteScheduleTask({
      path: { schedule: schedule.id, task: deleting.id, uuid }
    });
    pending = false;
    if (!res.response?.ok) {
      toast.error("Couldn't delete the task");
      return;
    }
    deleteOpen = false;
    toast.success('Task deleted');
    await onchange();
  };

  const toggle = async (
    key: 'enabled' | 'only_when_online',
    value: boolean
  ) => {
    patching = true;
    await onpatch(schedule, { [key]: value });
    patching = false;
  };

  const taskNumber = (t: TaskInfo) => tasks.indexOf(t) + 1;
</script>

<div class="grid gap-3">
  <div class="flex flex-wrap items-center gap-2">
    <Button
      variant="ghost"
      size="icon-sm"
      href={backHref}
      aria-label="Back to schedules"
      data-sveltekit-keepfocus
      data-sveltekit-noscroll
    >
      <ArrowLeft />
    </Button>
    <h2 class="truncate text-lg font-semibold">{schedule.name}</h2>
    {@render statusBadge(schedule)}
    {#if canWrite}
      <div class="ml-auto flex flex-wrap gap-2">
        <Button
          variant="outline"
          class="cursor-pointer"
          disabled={running}
          onclick={() => onrun(schedule)}
        >
          <Play />
          Run now
        </Button>
        <Button
          variant="outline"
          class="cursor-pointer"
          onclick={() => onedit(schedule)}
        >
          Edit
        </Button>
        <Button
          variant="outline"
          class="text-destructive hover:text-destructive cursor-pointer"
          onclick={() => ondelete(schedule)}
        >
          Delete
        </Button>
      </div>
    {/if}
  </div>

  {#if running && active !== null}
    <Alert.Root>
      <Info />
      <Alert.Title>Running task {active + 1} of {tasks.length}</Alert.Title>
      <Alert.Description>
        {#if activeTask}
          {activeTask.offset_seconds
            ? `Waiting ${activeTask.offset_seconds} s before "${activeTask.payload || ACTION_LABELS[activeTask.action]}".`
            : `Running "${activeTask.payload || ACTION_LABELS[activeTask.action]}".`}
        {/if}
        Run now is unavailable until the schedule finishes.
      </Alert.Description>
    </Alert.Root>
  {/if}

  <div class="grid items-start gap-3 lg:grid-cols-2">
    <Card.Root>
      <Card.Header>
        <Card.Title>Schedule</Card.Title>
        <Card.Description
          >{cronText(schedule.cron)}, server time (UTC).</Card.Description
        >
      </Card.Header>
      <Card.Content class="grid gap-4">
        <dl class="grid grid-cols-3 gap-2 sm:grid-cols-5">
          {#each CRON_FIELDS as [key, label] (key)}
            <div class="grid content-start gap-1">
              <dt class="text-muted-foreground text-xs">{label}</dt>
              <dd
                class="bg-muted/50 rounded-md border px-3 py-2 font-mono text-sm"
              >
                {schedule.cron[key]}
              </dd>
            </div>
          {/each}
        </dl>
        <Separator />
        <div class="flex items-center gap-3">
          <div class="grid grow gap-0.5">
            <span class="text-sm font-medium" id="sched-enabled">Enabled</span>
            <span class="text-muted-foreground text-xs"
              >Turn off to pause without deleting.</span
            >
          </div>
          <Switch
            aria-labelledby="sched-enabled"
            checked={schedule.enabled}
            disabled={!canWrite || patching}
            onCheckedChange={(v) => toggle('enabled', v)}
          />
        </div>
        <div class="flex items-center gap-3">
          <div class="grid grow gap-0.5">
            <span class="text-sm font-medium" id="sched-online"
              >Only when the server is online</span
            >
            <span class="text-muted-foreground text-xs"
              >Skips the run if the server is offline.</span
            >
          </div>
          <Switch
            aria-labelledby="sched-online"
            checked={schedule.only_when_online}
            disabled={!canWrite || patching}
            onCheckedChange={(v) => toggle('only_when_online', v)}
          />
        </div>
        <Separator />
        <dl class="grid grid-cols-2 gap-3 text-sm">
          <div>
            <dt class="text-muted-foreground text-xs">Last run</dt>
            <dd title={schedule.last_run_at?.toLocaleString()}>
              {lastRunText(schedule)}
            </dd>
          </div>
          <div>
            <dt class="text-muted-foreground text-xs">Next run</dt>
            <dd title={schedule.next_run_at?.toLocaleString()}>
              {nextRunText(schedule)}
            </dd>
          </div>
        </dl>
      </Card.Content>
    </Card.Root>

    <Card.Root>
      <Card.Header>
        <Card.Title>Tasks</Card.Title>
        <Card.Description
          >Run in order. Each waits its time offset first.</Card.Description
        >
        {#if canWrite}
          <Card.Action>
            <Button size="sm" class="cursor-pointer" onclick={() => openTask()}>
              <Plus />
              Add task
            </Button>
          </Card.Action>
        {/if}
      </Card.Header>
      <Card.Content>
        {#if tasks.length === 0}
          <Empty.Root class="border border-dashed">
            <Empty.Header>
              <Empty.Media variant="icon"><ListTodo /></Empty.Media>
              <Empty.Title>No tasks yet</Empty.Title>
              <Empty.Description>
                Add a command, power action or backup to run.
              </Empty.Description>
            </Empty.Header>
          </Empty.Root>
        {:else}
          <ol class="grid gap-2" aria-label="Tasks">
            {#each tasks as t, i (t.id)}
              <li
                class={cn(
                  'flex items-center gap-3 rounded-lg border p-3',
                  active === i && 'bg-muted'
                )}
                aria-current={active === i ? 'step' : undefined}
              >
                <span
                  class="bg-muted flex size-6 flex-none items-center justify-center rounded-full text-xs"
                  >{taskNumber(t)}</span
                >
                <div class="grid min-w-0 grow gap-1">
                  <div class="flex flex-wrap gap-1.5">
                    <Badge variant="secondary">{ACTION_LABELS[t.action]}</Badge>
                    {#if t.continue_on_failure}
                      <Badge variant="outline">Continues on failure</Badge>
                    {/if}
                    {#if active === i}
                      <Badge variant="outline"><Spinner />Waiting</Badge>
                    {/if}
                  </div>
                  {#if t.payload}
                    <span class="truncate font-mono text-sm" title={t.payload}
                      >{t.payload}</span
                    >
                  {/if}
                  <span class="text-muted-foreground text-xs"
                    >{offsetText(t)}</span
                  >
                </div>
                {#if canWrite}
                  <div class="flex flex-none gap-1">
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      class="cursor-pointer"
                      aria-label={`Edit task ${i + 1}`}
                      onclick={() => openTask(t)}
                    >
                      <Pencil />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      class="text-destructive hover:text-destructive cursor-pointer"
                      aria-label={`Delete task ${i + 1}`}
                      onclick={() => {
                        deleting = t;
                        deleteOpen = true;
                      }}
                    >
                      <Trash />
                    </Button>
                  </div>
                {/if}
              </li>
            {/each}
          </ol>
        {/if}
      </Card.Content>
    </Card.Root>
  </div>
</div>

{#if canWrite && taskOpen}
  <FormDialog
    title={editing ? 'Edit task' : 'Add task'}
    description={editing
      ? `Task ${taskNumber(editing)} of ${schedule.name}.`
      : `The task is added to the end of ${schedule.name}.`}
    confirm={editing ? 'Save task' : 'Add task'}
    schema={taskSchema}
    initialValue={editing
      ? {
          action: editing.action,
          continue_on_failure: editing.continue_on_failure,
          offset_seconds: editing.offset_seconds,
          payload: editing.payload
        }
      : undefined}
    onsubmit={submitTask}
    bind:open={taskOpen}
    bind:isLoading={taskLoading}
  >
    {#snippet children({ props })}
      <TaskFields {...props} noBackups={server.backup_limit === 0} />
    {/snippet}
  </FormDialog>
{/if}

<AlertDialog.Root bind:open={deleteOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Delete this task?</AlertDialog.Title>
      <AlertDialog.Description>
        {deleting ? ACTION_LABELS[deleting.action] : ''}
        {deleting?.payload ? `"${deleting.payload}"` : ''} is removed from {schedule.name}.
        This cannot be undone.
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
        Delete task
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
