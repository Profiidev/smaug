<script lang="ts">
  import FormInput from 'positron-components/components/form/form-input.svelte';
  import FormSwitch from 'positron-components/components/form/form-switch.svelte';
  import { Label } from 'positron-components/components/ui/dropdown-menu';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import type {
    FormValue,
    SuperForm
  } from 'positron-components/components/form/types';
  import type { generalSettings } from './schema.svelte';

  interface Props {
    readonly?: boolean;
    formData: SuperForm<FormValue<typeof generalSettings>>;
    disabled: boolean;
    secure: boolean;
  }

  let { readonly, secure, ...props }: Props = $props();
</script>

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
