<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormCheckbox from '@profidev/pleiades/components/form/form-checkbox.svelte';
  import FormTags from '@profidev/pleiades/components/form/form-tags.svelte';
  import FormTextarea from './FormTextarea.svelte';
  import { KNOWN_FEATURES } from './schema.svelte';
  import type { FormValue, SuperForm } from '@profidev/pleiades/components/form/types';
  import type { generalStage } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof generalStage>>;
    disabled: boolean;
  }

  let { ...props }: Props = $props();
</script>

<FormInput {...props} key="name" label="Egg Name" placeholder="Paper" />
<FormInput
  {...props}
  key="author"
  label="Author Email"
  placeholder="dev@example.com"
/>
<FormTextarea
  {...props}
  key="description"
  label="Description"
  rows={3}
  placeholder="What does this egg run?"
/>
<FormInput
  {...props}
  key="updateUrl"
  label="Update URL"
  placeholder="https://raw.githubusercontent.com/.../egg-paper.json"
/>

<div class="mt-2 grid gap-2">
  <span class="text-sm font-medium">Features</span>
  {#each KNOWN_FEATURES as feature (feature.key)}
    <FormCheckbox
      {...props}
      key={feature.formKey}
      label={`${feature.label} — ${feature.description}`}
      switchOrder
    />
  {/each}
</div>

<FormTags
  {...props}
  key="customFeatures"
  label="Custom feature flags"
  placeholder="Type a flag and press Enter"
/>
