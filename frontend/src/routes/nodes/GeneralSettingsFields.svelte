<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormSwitch from '@profidev/pleiades/components/form/form-switch.svelte';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import type {
    FormValue,
    SuperForm
  } from '@profidev/pleiades/components/form/types';
  import type { generalSettings } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof generalSettings>>;
    disabled: boolean;
  }

  let props: Props = $props();

  let form = $derived(props.formData.form);
</script>

<FormInput {...props} key="name" label="Node Name" placeholder="Enter name" />
<FormInput
  {...props}
  key="address"
  label="Node Address (include port if needed)"
  placeholder="Enter address (e.g., IP or domain)"
/>
{#if !$form.secure}
  <div class="flex items-center text-sm font-medium text-orange-500">
    Only use HTTP on private networks!
    <TriangleAlert class="ml-auto" />
  </div>
{/if}
<FormSwitch {...props} key="secure" label="Use Secure Connection (HTTPS)" />
