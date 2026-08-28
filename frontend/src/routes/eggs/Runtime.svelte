<script lang="ts">
  import BaseForm from '@profidev/pleiades/components/form/base-form.svelte';
  import { type FormValue } from '@profidev/pleiades/components/form/types';
  import type { ComponentProps, Snippet } from 'svelte';
  import { runtimeStage } from './schema.svelte';
  import RuntimeFields from './RuntimeFields.svelte';

  interface Props {
    initialValue?: FormValue<typeof runtimeStage>;
    // @ts-ignore
    onsubmit: ComponentProps<BaseForm<any>>['onsubmit'];
    footer: Snippet<[{ isLoading: boolean; isError: boolean }]>;
    isLoading: boolean;
  }

  let { initialValue, onsubmit, footer, isLoading }: Props = $props();

  let form: BaseForm<typeof runtimeStage> | undefined = $state();

  export const getValue = () => {
    return form?.getValue();
  };
</script>

<BaseForm
  schema={runtimeStage}
  {onsubmit}
  {footer}
  {initialValue}
  bind:this={form}
  bind:isLoading
>
  {#snippet children({ props })}
    <RuntimeFields {...props} />
  {/snippet}
</BaseForm>
