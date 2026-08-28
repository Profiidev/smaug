<script lang="ts">
  import BaseForm from '@profidev/pleiades/components/form/base-form.svelte';
  import { type FormValue } from '@profidev/pleiades/components/form/types';
  import type { ComponentProps, Snippet } from 'svelte';
  import { generalStage } from './schema.svelte';
  import GeneralFields from './GeneralFields.svelte';

  interface Props {
    initialValue?: FormValue<typeof generalStage>;
    // @ts-ignore
    onsubmit: ComponentProps<BaseForm<any>>['onsubmit'];
    footer: Snippet<[{ isLoading: boolean; isError: boolean }]>;
    isLoading: boolean;
  }

  let { initialValue, onsubmit, footer, isLoading }: Props = $props();

  let form: BaseForm<typeof generalStage> | undefined = $state();

  export const getValue = () => {
    return form?.getValue();
  };
</script>

<BaseForm
  schema={generalStage}
  {onsubmit}
  {footer}
  {initialValue}
  bind:this={form}
  bind:isLoading
>
  {#snippet children({ props })}
    <GeneralFields {...props} />
  {/snippet}
</BaseForm>
