<script lang="ts">
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import FormInput from 'positron-components/components/form/form-input.svelte';
  import FormSwitch from 'positron-components/components/form/form-switch.svelte';
  import { type FormValue } from 'positron-components/components/form/types';
  import type { ComponentProps, Snippet } from 'svelte';
  import { generalSettings } from './schema.svelte';
  import { Label } from 'positron-components/components/ui/dropdown-menu';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';

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
    <FormInput
      {...props}
      key="name"
      label="Node Name"
      placeholder="Enter name"
      {readonly}
    />
    <FormInput
      {...props}
      key="address"
      label="Node Address (include port if needed)"
      placeholder="Enter address (e.g., IP or domain)"
      {readonly}
    />
    {#if !secure}
      <div class="flex items-center">
        <Label class="p-0 font-medium text-orange-500"
          >Only use HTTP on private networks!</Label
        >
        <TriangleAlert class="ml-auto text-orange-500" />
      </div>
    {/if}
    <FormSwitch
      {...props}
      key="secure"
      label="Use Secure Connection (HTTPS)"
      onCheckedChange={(v) => (secure = v)}
      disabled={readonly}
    />
  {/snippet}
</BaseForm>
