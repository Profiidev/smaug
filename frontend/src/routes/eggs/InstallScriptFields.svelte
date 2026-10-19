<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormTextarea from '@profidev/pleiades/components/form/form-textarea.svelte';
  import type {
    FormValue,
    SuperForm
  } from '@profidev/pleiades/components/form/types';
  import type { installStage } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof installStage>>;
    disabled: boolean;
  }

  let { ...props }: Props = $props();
</script>

<div class="grid gap-4 md:grid-cols-2">
  <FormInput
    {...props}
    key="scriptsContainer"
    label="Install Container Image"
    class="font-mono"
    placeholder="ghcr.io/pterodactyl/installers:alpine"
  />
  <FormInput
    {...props}
    key="scriptsEntrypoint"
    label="Entrypoint Shell"
    class="font-mono"
    placeholder="ash / bash"
  />
</div>
<div class="grid gap-1">
  <FormTextarea
    {...props}
    key="scriptsScript"
    label="Install Script"
    rows={14}
    spellcheck={false}
    class="font-mono text-xs"
    placeholder={'Leave empty to run nothing during install. Shell script — supports the same {{VAR}} templating as the startup command.'}
  />
  <p class="text-muted-foreground text-xs">
    Runs once, as root, in the install container. The server files are mounted
    at /mnt/server.
  </p>
</div>
