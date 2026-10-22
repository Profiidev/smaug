<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormSwitch from '@profidev/pleiades/components/form/form-switch.svelte';
  import FormTextarea from '@profidev/pleiades/components/form/form-textarea.svelte';
  import type {
    FormValue,
    SuperForm
  } from '@profidev/pleiades/components/form/types';
  import * as Select from '@profidev/pleiades/components/ui/select';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import type { ScheduleAction } from '$lib/client';
  import { IGNORE_PLACEHOLDER } from './backups';
  import { ACTION_LABELS, POWER_ACTIONS, type taskSchema } from './schedules';

  interface Props {
    formData: SuperForm<FormValue<typeof taskSchema>>;
    disabled: boolean;
    /** Backups are disabled for the server (limit 0). */
    noBackups: boolean;
  }

  let { noBackups, ...props }: Props = $props();

  let form = $derived(props.formData.form);
  let errors = $derived(props.formData.errors);
  let actions = $derived(
    (Object.keys(ACTION_LABELS) as ScheduleAction[]).filter(
      (a) => !(noBackups && a === 'backup')
    )
  );

  const pickAction = (action: string) => {
    if (action === $form.action) return;
    $form.action = action as ScheduleAction;
    $form.payload = action === 'power' ? 'restart' : '';
  };
</script>

<div class="grid gap-1">
  <Label for="task-action">Action</Label>
  <Select.Root
    type="single"
    value={$form.action}
    disabled={props.disabled}
    onValueChange={pickAction}
  >
    <Select.Trigger id="task-action" class="w-full cursor-pointer sm:w-fit">
      {ACTION_LABELS[$form.action]}
    </Select.Trigger>
    <Select.Content>
      {#each actions as a (a)}
        <Select.Item value={a}>{ACTION_LABELS[a]}</Select.Item>
      {/each}
    </Select.Content>
  </Select.Root>
</div>

<div class="grid gap-1">
  {#if $form.action === 'power'}
    <Label for="task-power">Power action</Label>
    <Select.Root
      type="single"
      value={$form.payload}
      disabled={props.disabled}
      onValueChange={(v) => ($form.payload = v)}
    >
      <Select.Trigger id="task-power" class="w-full cursor-pointer font-mono">
        {$form.payload || 'Pick one'}
      </Select.Trigger>
      <Select.Content>
        {#each POWER_ACTIONS as p (p)}
          <Select.Item value={p} class="font-mono">{p}</Select.Item>
        {/each}
      </Select.Content>
    </Select.Root>
    {#if $errors.payload}
      <p class="text-destructive text-sm">{$errors.payload[0]}</p>
    {/if}
  {:else if $form.action === 'backup'}
    <FormTextarea
      {...props}
      key="payload"
      label="Ignored files"
      rows={3}
      class="font-mono"
      placeholder={IGNORE_PLACEHOLDER}
    />
    <p class="text-muted-foreground text-xs">
      Optional. One pattern per line, .gitignore style, on top of the server's
      own ignore files.
    </p>
  {:else}
    <FormInput
      {...props}
      key="payload"
      label="Command"
      class="font-mono"
      placeholder="say Restarting in 5 minutes"
      autocomplete="off"
    />
    <p class="text-muted-foreground text-xs">
      Sent to the server console, without a leading slash.
    </p>
  {/if}
</div>

<div class="grid gap-1">
  <FormInput
    {...props}
    key="offset_seconds"
    label="Time offset (seconds)"
    type="number"
    min={0}
    class="font-mono"
  />
  <p class="text-muted-foreground text-xs">
    How long to wait after the previous task. 0 runs it right away.
  </p>
</div>

<div class="grid gap-0.5">
  <FormSwitch
    {...props}
    key="continue_on_failure"
    label="Continue on failure"
  />
  <p class="text-muted-foreground text-xs">
    Run the next task even if this one fails.
  </p>
</div>
