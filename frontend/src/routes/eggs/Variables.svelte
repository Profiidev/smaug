<script lang="ts">
  import BaseForm from '@profidev/pleiades/components/form/base-form.svelte';
  import { type FormValue } from '@profidev/pleiades/components/form/types';
  import type { ComponentProps, Snippet } from 'svelte';
  import { variablesStage } from './schema.svelte';
  import VariablesFields from './VariablesFields.svelte';

  interface Props {
    initialValue?: FormValue<typeof variablesStage>;
    // @ts-ignore
    onsubmit: ComponentProps<BaseForm<any>>['onsubmit'];
    footer: Snippet<[{ isLoading: boolean; isError: boolean }]>;
    isLoading: boolean;
  }

  let { initialValue, onsubmit, footer, isLoading }: Props = $props();

  let form: BaseForm<typeof variablesStage> | undefined = $state();

  export const getValue = () => {
    return form?.getValue();
  };
</script>

<BaseForm
  schema={variablesStage}
  {onsubmit}
  {footer}
  {initialValue}
  bind:this={form}
  bind:isLoading
>
  {#snippet children({ props })}
    <VariablesFields {...props} />
  {/snippet}
</BaseForm>
