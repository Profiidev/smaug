<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormSwitch from '@profidev/pleiades/components/form/form-switch.svelte';
  import FormTags from '@profidev/pleiades/components/form/form-tags.svelte';
  import FormTextarea from './FormTextarea.svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import Plus from '@lucide/svelte/icons/plus';
  import Trash from '@lucide/svelte/icons/trash';
  import type { FormValue, SuperForm } from '@profidev/pleiades/components/form/types';
  import type { runtimeStage } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof runtimeStage>>;
    disabled: boolean;
  }

  let { ...props }: Props = $props();

  let formData = $derived(props.formData.form);

  const addDockerRow = () => {
    $formData.dockerImages = [...$formData.dockerImages, { image: '', label: '' }];
  };

  const removeDockerRow = (index: number) => {
    $formData.dockerImages = $formData.dockerImages.filter((_, i) => i !== index);
  };
</script>

<div>
  <span class="text-sm font-medium">Docker Images</span>
  <p class="text-muted-foreground mt-1 text-xs">
    Display-name → image reference. At least one is required.
  </p>
  {#each $formData.dockerImages as _, i (i)}
    <div class="mt-2 flex items-start gap-2">
      <div class="flex-1">
        <FormInput
          {...props}
          key={`dockerImages[${i}].label`}
          label="Label"
          placeholder="Java 21"
        />
      </div>
      <div class="flex-1">
        <FormInput
          {...props}
          key={`dockerImages[${i}].image`}
          label="Image"
          placeholder="ghcr.io/pterodactyl/yolks:java_21"
        />
      </div>
      <Button
        size="icon-sm"
        variant="ghost"
        type="button"
        class="mt-6"
        onclick={() => removeDockerRow(i)}
      >
        <Trash />
      </Button>
    </div>
  {/each}
  <Button size="sm" variant="outline" type="button" class="mt-2" onclick={addDockerRow}>
    <Plus />
    Add image
  </Button>
</div>

<FormTextarea
  {...props}
  key="startup"
  label="Startup Command"
  rows={3}
  class="font-mono text-xs"
  placeholder={'java -Xms128M -Xmx{{SERVER_MEMORY}}M -jar {{SERVER_JARFILE}}'}
/>

<FormInput
  {...props}
  key="configStartupDone"
  label="Done String(s)"
  placeholder={'")! For help, type \\"help\\""'}
/>

<FormTags
  {...props}
  key="configUserInteraction"
  label="Expected user-interaction prompts"
  placeholder="Do you agree?"
/>

<FormSwitch
  {...props}
  key="configStripAnsi"
  label="Strip ANSI color codes before matching"
/>

<FormInput {...props} key="configStop" label="Stop Command" placeholder="stop" />
