<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormSelect from '@profidev/pleiades/components/form/form-select.svelte';
  import FormSwitch from '@profidev/pleiades/components/form/form-switch.svelte';
  import { advancedSettings, units } from './schema.svelte';
  import type {
    FormValue,
    SuperForm
  } from '@profidev/pleiades/components/form/types';

  interface Props {
    formData: SuperForm<FormValue<typeof advancedSettings>>;
    disabled: boolean;
  }

  let props: Props = $props();

  let form = $derived(props.formData.form);

  const unitOptions = Object.keys(units).map((unit) => ({
    label: unit,
    value: unit
  }));
</script>

<FormSwitch {...props} key="cpu_unlimit" label="Unlimited CPU" />
{#if !$form.cpu_unlimit}
  <FormInput
    {...props}
    key="cpu_limit"
    label="CPU Limit (in millicores)"
    placeholder="Enter CPU limit"
    type="number"
  />
{/if}
<FormSwitch {...props} key="memory_unlimit" label="Unlimited Memory" />
{#if !$form.memory_unlimit}
  <div class="grid w-full grid-cols-[1fr_auto] gap-2">
    <FormInput
      {...props}
      key="memory_limit"
      label="Available Memory"
      placeholder="Enter amount of memory"
      type="number"
    />
    <FormSelect
      {...props}
      class="w-16"
      key="memory_limit_unit"
      label="Unit"
      single={true}
      data={unitOptions}
    />
  </div>
{/if}
<FormSwitch {...props} key="storage_unlimit" label="Unlimited Storage" />
{#if !$form.storage_unlimit}
  <div class="grid w-full grid-cols-[1fr_auto] gap-2">
    <FormInput
      {...props}
      key="storage_size"
      label="Available Storage Space"
      placeholder="Enter amount of storage"
      type="number"
    />
    <FormSelect
      {...props}
      class="w-16"
      key="storage_size_unit"
      label="Unit"
      single={true}
      data={unitOptions}
    />
  </div>
{/if}
