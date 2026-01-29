<script lang="ts">
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import { type FormValue } from 'positron-components/components/form/types';
  import type { ComponentProps, Snippet } from 'svelte';
  import { advancedSettings } from './schema.svelte';
  import AdvancedSettingsFields from './AdvancedSettingsFields.svelte';

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
    <AdvancedSettingsFields
      {cpuUnlimit}
      {memoryUnlimit}
      {storageUnlimit}
      {readonly}
      {...props}
    />
  {/snippet}
</BaseForm>
