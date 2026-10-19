<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Form from '@profidev/pleiades/components/ui/form';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import * as Select from '@profidev/pleiades/components/ui/select';
  import * as ToggleGroup from '@profidev/pleiades/components/ui/toggle-group';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import Plus from '@lucide/svelte/icons/plus';
  import Trash from '@lucide/svelte/icons/trash';
  import X from '@lucide/svelte/icons/x';
  import { FIND_MODES } from './schema.svelte';
  import type {
    FormValue,
    SuperForm
  } from '@profidev/pleiades/components/form/types';
  import type { configStage } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof configStage>>;
    disabled: boolean;
    /** Config file index. */
    i: number;
    /** Find entry index within the file. */
    j: number;
    onremove: () => void;
  }

  let { formData: form, disabled, i, j, onremove }: Props = $props();

  let formData = $derived(form.form);
  let entry = $derived($formData.configFiles[i].find[j]);
  let mode = $derived(entry.mode[0]);
  let path = $derived(`configFiles[${i}].find[${j}]` as const);

  const MODE_LABELS = {
    boolean: 'Boolean',
    map: 'Map',
    number: 'Number',
    string: 'String'
  } as const;

  const setMode = (value: string) => {
    const next = FIND_MODES.find((m) => m === value);
    // A single toggle group emits '' when the active item is clicked again.
    if (!next || next === mode) return;
    $formData.configFiles[i].find[j].mode = [next];
    if (next === 'boolean' && entry.value !== 'true') {
      $formData.configFiles[i].find[j].value = 'false';
    }
  };

  const setValue = (value: string) => {
    $formData.configFiles[i].find[j].value = value;
  };

  const setMap = (map: typeof entry.map) => {
    $formData.configFiles[i].find[j].map = map;
  };
</script>

<div class="bg-background grid gap-3 rounded-lg border p-3">
  <div class="flex flex-wrap items-start gap-2">
    <div class="min-w-0 flex-[2_1_200px]">
      <FormInput
        formData={form}
        {disabled}
        key={`${path}.key`}
        label="Key"
        class="font-mono"
        placeholder="server-port"
      />
    </div>
    <ToggleGroup.Root
      type="single"
      variant="outline"
      size="sm"
      class="mt-6"
      aria-label="Type"
      {disabled}
      value={mode}
      onValueChange={setMode}
    >
      {#each FIND_MODES as m (m)}
        <ToggleGroup.Item value={m}>{MODE_LABELS[m]}</ToggleGroup.Item>
      {/each}
    </ToggleGroup.Root>
    <Button
      size="icon-sm"
      variant="destructive"
      type="button"
      class="mt-6 ml-auto"
      aria-label="Remove find entry"
      {disabled}
      onclick={onremove}
    >
      <Trash />
    </Button>
  </div>

  {#if mode === 'map'}
    <div class="grid gap-2">
      <span class="text-muted-foreground text-xs">
        If the current value matches (prefix <code>regex:</code> for a regex), replace
        it.
      </span>
      {#each entry.map as _, k (k)}
        <div class="flex flex-wrap items-start gap-2 sm:flex-nowrap">
          <div class="min-w-0 flex-[1_1_140px]">
            <FormInput
              formData={form}
              {disabled}
              key={`${path}.map[${k}].match`}
              label="Match"
              class="font-mono"
              placeholder="127.0.0.1"
            />
          </div>
          <ArrowRight
            class="text-muted-foreground mt-8 hidden size-4 flex-none sm:block"
          />
          <div class="min-w-0 flex-[3_1_220px]">
            <FormInput
              formData={form}
              {disabled}
              key={`${path}.map[${k}].replace`}
              label="Replace with"
              class="font-mono"
              placeholder={'{{server.allocations.default.ip}}'}
            />
          </div>
          <Button
            size="icon-sm"
            variant="destructive"
            type="button"
            class="mt-6"
            aria-label="Remove pair"
            {disabled}
            onclick={() => setMap(entry.map.filter((_, m) => m !== k))}
          >
            <X />
          </Button>
        </div>
      {/each}
      <div>
        <Button
          size="sm"
          variant="ghost"
          type="button"
          {disabled}
          onclick={() => setMap([...entry.map, { match: '', replace: '' }])}
        >
          <Plus />
          Add pair
        </Button>
      </div>
    </div>
  {:else if mode === 'string'}
    <FormInput
      formData={form}
      {disabled}
      key={`${path}.value`}
      label="Value"
      class="font-mono"
      placeholder={'{{server.allocations.default.port}}'}
    />
  {:else}
    <!-- Number and boolean controls produce non-string values, so they write the string field by hand. -->
    <Form.Field {form} name={`${path}.value`} class="grid gap-1">
      <Form.Control>
        {#snippet children({ props })}
          <Form.Label>Value</Form.Label>
          {#if mode === 'number'}
            <Input
              {...props}
              type="number"
              class="font-mono"
              placeholder="25565"
              {disabled}
              bind:value={
                () => entry.value, (v) => setValue(v == null ? '' : String(v))
              }
            />
          {:else}
            <Select.Root
              type="single"
              {disabled}
              value={entry.value}
              onValueChange={setValue}
            >
              <Select.Trigger {...props} class="w-full font-mono">
                {entry.value || 'Choose a value'}
              </Select.Trigger>
              <Select.Content>
                <Select.Item value="true">true</Select.Item>
                <Select.Item value="false">false</Select.Item>
              </Select.Content>
            </Select.Root>
          {/if}
        {/snippet}
      </Form.Control>
      <Form.FieldErrors />
    </Form.Field>
  {/if}
</div>
