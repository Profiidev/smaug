<script lang="ts">
  import BaseForm from '@profidev/pleiades/components/form/base-form.svelte';
  import { type FormValue } from '@profidev/pleiades/components/form/types';
  import type { ComponentProps, Snippet } from 'svelte';
  import { configStage } from './schema.svelte';
  import ConfigFields from './ConfigFields.svelte';

  interface Props {
    initialValue?: FormValue<typeof configStage>;
    // @ts-ignore
    onsubmit: ComponentProps<BaseForm<any>>['onsubmit'];
    footer: Snippet<[{ isLoading: boolean; isError: boolean }]>;
    isLoading: boolean;
  }

  let { initialValue, onsubmit, footer, isLoading }: Props = $props();

  let form: BaseForm<typeof configStage> | undefined = $state();

  export const getValue = () => {
    return form?.getValue();
  };
</script>

<BaseForm
  schema={configStage}
  {onsubmit}
  {footer}
  {initialValue}
  bind:this={form}
  bind:isLoading
>
  {#snippet children({ props })}
    <ConfigFields {...props} />
  {/snippet}
</BaseForm>
