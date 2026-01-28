<script lang="ts">
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import FormInput from 'positron-components/components/form/form-input.svelte';
  import FormSelect from 'positron-components/components/form/form-select.svelte';
  import { type FormValue } from 'positron-components/components/form/types';
  import type { ComponentProps, Snippet } from 'svelte';
  import { advancedSettings, units } from './schema.svelte';
  import FormSwitch from 'positron-components/components/form/form-switch.svelte';

  interface Props {
    initialValue?: FormValue<typeof advancedSettings>;
    onsubmit: ComponentProps<typeof BaseForm>['onsubmit'];
    footer: Snippet<[{ isLoading: boolean }]>;
    isLoading: boolean;
    readonly?: boolean;
  }

  let { initialValue, onsubmit, footer, isLoading, readonly }: Props = $props();

  let form: BaseForm<typeof advancedSettings> | undefined = $state();
  // svelte-ignore state_referenced_locally
  let cpuUnlimit: boolean = $state(initialValue?.cpu_unlimit ?? true);
  // svelte-ignore state_referenced_locally
  let memoryUnlimit: boolean = $state(initialValue?.memory_unlimit ?? true);
  // svelte-ignore state_referenced_locally
  let storageUnlimit: boolean = $state(initialValue?.storage_unlimit ?? true);

  export const getValue = () => {
    return form?.getValue();
  };
</script>

<BaseForm
  schema={advancedSettings}
  {onsubmit}
  {footer}
  {initialValue}
  bind:this={form}
  bind:isLoading
>
  {#snippet children({ props })}
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
  {/snippet}
</BaseForm>
