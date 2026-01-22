<script lang="ts">
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import FormInput from 'positron-components/components/form/form-input.svelte';
  import FormSwitch from 'positron-components/components/form/form-switch.svelte';
  import { type FormValue } from 'positron-components/components/form/types';
  import type { ComponentProps, Snippet } from 'svelte';
  import { generalInformation } from './schema.svelte';

  interface Props {
    initialValue?: FormValue<typeof generalInformation>;
    onsubmit: ComponentProps<typeof BaseForm>['onsubmit'];
    footer: Snippet<[{ isLoading: boolean }]>;
    isLoading: boolean;
  }

  let { initialValue, onsubmit, footer, isLoading }: Props = $props();

  let form: BaseForm<typeof generalInformation> | undefined = $state();

  export const getValue = () => {
    return form?.getValue();
  };
</script>

<BaseForm
  schema={generalInformation}
  {onsubmit}
  {footer}
  {initialValue}
  bind:this={form}
  bind:isLoading
>
  {#snippet children({ props })}
    <FormInput
      {...props}
      key="name"
      label="Node Name"
      placeholder="Enter name"
    />
    <FormInput
      {...props}
      key="address"
      label="Node Address (include port if needed)"
      placeholder="Enter address (e.g., IP or domain)"
    />
    <FormSwitch {...props} key="secure" label="Use Secure Connection (HTTPS)" />
  {/snippet}
</BaseForm>
