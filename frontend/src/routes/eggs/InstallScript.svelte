<script lang="ts">
  import BaseForm from '@profidev/pleiades/components/form/base-form.svelte';
  import type {
    FormValue,
    SubmitHandler
  } from '@profidev/pleiades/components/form/types';
  import type { Snippet } from 'svelte';
  import { SECTION_DESCRIPTIONS, installStage } from './schema.svelte';
  import InstallScriptFields from './InstallScriptFields.svelte';

  interface Props {
    initialValue?: FormValue<typeof installStage>;
    onsubmit: SubmitHandler<typeof installStage>;
    footer: Snippet<[{ isLoading: boolean; isError: boolean }]>;
    isLoading?: boolean;
  }

  let {
    initialValue,
    onsubmit,
    footer,
    isLoading = $bindable(false)
  }: Props = $props();

  let form: BaseForm<typeof installStage> | undefined = $state();

  export const getValue = () => form?.getValue();
</script>

<BaseForm
  schema={installStage}
  {onsubmit}
  {footer}
  {initialValue}
  bind:this={form}
  bind:isLoading
  class="flex min-h-0 flex-col gap-4"
>
  {#snippet children({ props })}
    <p class="text-muted-foreground -mt-4 text-sm">
      {SECTION_DESCRIPTIONS.install}
    </p>
    <!-- Plain block scroller: a sized grid/flex parent would squash overflow-hidden cards. -->
    <div class="-mx-1 min-h-0 overflow-y-auto px-1 pb-1">
      <div class="grid gap-4">
        <InstallScriptFields {...props} />
      </div>
    </div>
  {/snippet}
</BaseForm>
