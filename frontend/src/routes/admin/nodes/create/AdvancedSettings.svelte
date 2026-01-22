<script lang="ts">
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import FormInput from 'positron-components/components/form/form-input.svelte';
  import FormSelect from 'positron-components/components/form/form-select.svelte';
  import { type FormValue } from 'positron-components/components/form/types';
  import type { ComponentProps, Snippet } from 'svelte';
  import { resources, units } from './schema.svelte';

  interface Props {
    initialValue?: FormValue<typeof resources>;
    onsubmit: ComponentProps<typeof BaseForm>['onsubmit'];
    footer: Snippet<[{ isLoading: boolean }]>;
    isLoading: boolean;
  }

  let { initialValue, onsubmit, footer, isLoading }: Props = $props();

  let form: BaseForm<typeof resources> | undefined = $state();

  export const getValue = () => {
    return form?.getValue();
  };
</script>

<BaseForm
  schema={resources}
  {onsubmit}
  {footer}
  {initialValue}
  bind:this={form}
  bind:isLoading
>
  {#snippet children({ props })}
    <div class="flex w-full gap-2">
      <FormInput
        {...props}
        class="w-89"
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
        data={Object.keys(units).map((unit) => ({ value: unit, label: unit }))}
      />
    </div>
  {/snippet}
</BaseForm>
