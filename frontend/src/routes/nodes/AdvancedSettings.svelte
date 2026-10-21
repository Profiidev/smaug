<script lang="ts">
  import BaseForm from '@profidev/pleiades/components/form/base-form.svelte';
  import type {
    FormValue,
    SubmitHandler
  } from '@profidev/pleiades/components/form/types';
  import type { Snippet } from 'svelte';
  import { advancedSettings } from './schema.svelte';
  import AdvancedSettingsFields from './AdvancedSettingsFields.svelte';

  interface Props {
    initialValue?: FormValue<typeof advancedSettings>;
    onsubmit: SubmitHandler<typeof advancedSettings>;
    footer: Snippet<[{ isLoading: boolean; isError: boolean }]>;
    isLoading?: boolean;
  }

  let {
    initialValue,
    onsubmit,
    footer,
    isLoading = $bindable(false)
  }: Props = $props();

  let form: BaseForm<typeof advancedSettings> | undefined = $state();

  export const getValue = () => form?.getValue();
</script>

<BaseForm
  schema={advancedSettings}
  {onsubmit}
  {footer}
  {initialValue}
  bind:this={form}
  bind:isLoading
  class="flex min-h-0 flex-col gap-4"
>
  {#snippet children({ props })}
    <!-- Plain block scroller: a sized grid/flex parent would squash the fields. -->
    <div class="-mx-1 min-h-0 overflow-y-auto px-1 pb-1">
      <div class="grid gap-4">
        <AdvancedSettingsFields {...props} />
      </div>
    </div>
  {/snippet}
</BaseForm>
