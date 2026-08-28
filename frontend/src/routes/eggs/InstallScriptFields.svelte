<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormTextarea from './FormTextarea.svelte';
  import type { FormValue, SuperForm } from '@profidev/pleiades/components/form/types';
  import type { installStage } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof installStage>>;
    disabled: boolean;
  }

  let { ...props }: Props = $props();
</script>

<FormTextarea
  {...props}
  key="scriptsScript"
  label="Install Script"
  rows={12}
  class="font-mono text-xs"
  placeholder={'Leave empty to run nothing during install. Shell script — supports the same {{VAR}} templating as the startup command.'}
/>
<div class="grid grid-cols-1 gap-3 md:grid-cols-2">
  <FormInput
    {...props}
    key="scriptsContainer"
    label="Install Container Image"
    placeholder="ghcr.io/pterodactyl/installers:alpine"
  />
  <FormInput
    {...props}
    key="scriptsEntrypoint"
    label="Entrypoint Shell"
    placeholder="ash / bash"
  />
</div>
