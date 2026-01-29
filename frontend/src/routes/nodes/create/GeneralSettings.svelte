<script lang="ts">
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import { type FormValue } from 'positron-components/components/form/types';
  import type { ComponentProps, Snippet } from 'svelte';
  import { generalSettings } from './schema.svelte';
  import GeneralSettingsFields from './GeneralSettingsFields.svelte';

  interface Props {
    initialValue?: FormValue<typeof generalSettings>;
    onsubmit: ComponentProps<typeof BaseForm>['onsubmit'];
    footer: Snippet<[{ isLoading: boolean }]>;
    isLoading: boolean;
    readonly?: boolean;
  }

  let { initialValue, onsubmit, footer, isLoading, readonly }: Props = $props();

  let form: BaseForm<typeof generalSettings> | undefined = $state();
  // svelte-ignore state_referenced_locally
  let secure = $state(initialValue?.secure ?? true);

  export const getValue = () => {
    return form?.getValue();
  };
</script>

<BaseForm
  schema={generalSettings}
  {onsubmit}
  {footer}
  {initialValue}
  bind:this={form}
  bind:isLoading
>
  {#snippet children({ props })}
    <GeneralSettingsFields {secure} {readonly} {...props} />
  {/snippet}
</BaseForm>
