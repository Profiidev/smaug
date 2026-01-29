<script lang="ts">
  import FormInput from 'positron-components/components/form/form-input.svelte';
  import FormSelect from 'positron-components/components/form/form-select.svelte';
  import FormSwitch from 'positron-components/components/form/form-switch.svelte';
  import { advancedSettings, units } from './schema.svelte';
  import type {
    FormValue,
    SuperForm
  } from 'positron-components/components/form/types';

  interface Props {
    cpuUnlimit: boolean;
    memoryUnlimit: boolean;
    storageUnlimit: boolean;
    readonly?: boolean;
    formData: SuperForm<FormValue<typeof advancedSettings>>;
    disabled: boolean;
  }

  let { cpuUnlimit, memoryUnlimit, storageUnlimit, readonly, ...props }: Props =
    $props();
</script>

<FormSwitch
  {...props}
  key="cpu_unlimit"
  label="Unlimited CPU"
  onCheckedChange={(v) => (cpuUnlimit = v)}
  disabled={readonly}
/>
{#if !cpuUnlimit}
  <FormInput
    {...props}
    key="cpu_limit"
    label="CPU Limit (in millicores)"
    placeholder="Enter CPU limit"
    type="number"
    {readonly}
  />
{/if}
<FormSwitch
  {...props}
  key="memory_unlimit"
  label="Unlimited Memory"
  onCheckedChange={(v) => (memoryUnlimit = v)}
  disabled={readonly}
/>
{#if !memoryUnlimit}
  <div class="grid w-full grid-cols-[1fr_auto] gap-2">
    <FormInput
      {...props}
      key="memory_limit"
      label="Available Memory"
      placeholder="Enter amount of memory"
      type="number"
      {readonly}
    />
    <FormSelect
      {...props}
      class="w-16"
      key="memory_limit_unit"
      label="Unit"
      single={true}
      data={Object.keys(units).map((unit) => ({
        value: unit,
        label: unit
      }))}
      {readonly}
    />
  </div>
{/if}
<FormSwitch
  {...props}
  key="storage_unlimit"
  label="Unlimited Storage"
  onCheckedChange={(v) => (storageUnlimit = v)}
  disabled={readonly}
/>
{#if !storageUnlimit}
  <div class="grid w-full grid-cols-[1fr_auto] gap-2">
    <FormInput
      {...props}
      key="storage_size"
      label="Available Storage Space"
      placeholder="Enter amount of storage"
      type="number"
      {readonly}
    />
    <FormSelect
      {...props}
      class="w-16"
      key="storage_size_unit"
      label="Unit"
      single={true}
      data={Object.keys(units).map((unit) => ({
        value: unit,
        label: unit
      }))}
      {readonly}
    />
  </div>
{/if}
