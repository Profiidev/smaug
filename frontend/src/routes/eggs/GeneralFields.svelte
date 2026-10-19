<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormTags from '@profidev/pleiades/components/form/form-tags.svelte';
  import FormTextarea from '@profidev/pleiades/components/form/form-textarea.svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import { Switch } from '@profidev/pleiades/components/ui/switch';
  import * as Item from '@profidev/pleiades/components/ui/item';
  import Egg from '@lucide/svelte/icons/egg';
  import { KNOWN_FEATURES } from './schema.svelte';
  import type {
    FormValue,
    SuperForm
  } from '@profidev/pleiades/components/form/types';
  import type { generalStage } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof generalStage>>;
    disabled: boolean;
  }

  let { ...props }: Props = $props();

  let formData = $derived(props.formData.form);

  const readIcon = (file: File | undefined) => {
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      $formData.icon = reader.result as string;
    };
    reader.readAsDataURL(file);
  };

  const onDrop = (e: DragEvent) => {
    e.preventDefault();
    if (props.disabled) return;
    const file = e.dataTransfer?.files[0];
    if (file?.type.startsWith('image/')) readIcon(file);
  };
</script>

<div class="grid gap-4 md:grid-cols-2">
  <div class="grid gap-1">
    <FormInput {...props} key="name" label="Egg Name" placeholder="Paper" />
    <p class="text-muted-foreground text-xs">Up to 255 characters.</p>
  </div>
  <div class="grid gap-1">
    <FormInput
      {...props}
      key="author"
      label="Author Email"
      type="email"
      placeholder="dev@example.com"
    />
    <p class="text-muted-foreground text-xs">
      Contact email for whoever maintains this egg.
    </p>
  </div>
</div>
<FormTextarea
  {...props}
  key="description"
  label="Description"
  rows={3}
  placeholder="What does this egg run?"
/>
<div class="grid gap-1">
  <FormInput
    {...props}
    key="updateUrl"
    label="Update URL"
    class="font-mono"
    placeholder="https://raw.githubusercontent.com/.../egg-paper.json"
  />
  <p class="text-muted-foreground text-xs">
    Optional. Where the panel checks for a newer version of this egg.
  </p>
</div>
<FormTags
  {...props}
  key="tags"
  label="Tags"
  placeholder="Type a tag and press Enter"
/>

<div class="grid gap-2">
  <Label for="egg-icon">Icon</Label>
  <div class="flex flex-wrap items-center gap-3">
    <div
      class="bg-muted text-muted-foreground flex size-16 flex-none items-center justify-center overflow-hidden rounded-xl border"
    >
      {#if $formData.icon}
        <img
          src={$formData.icon}
          alt="Egg icon"
          class="size-full object-cover"
        />
      {:else}
        <Egg class="size-7" />
      {/if}
    </div>
    <div
      role="group"
      aria-label="Icon drop zone"
      class="border-input flex min-w-0 flex-[1_1_280px] flex-wrap items-center justify-between gap-2 rounded-xl border border-dashed px-4 py-3"
      class:opacity-50={props.disabled}
      ondragover={(e) => e.preventDefault()}
      ondrop={onDrop}
    >
      <div class="grid gap-0.5">
        <span class="text-sm font-medium">Drop an image here or browse</span>
        <span class="text-muted-foreground text-xs">
          PNG, JPEG or WebP. Stored inside the egg as base64.
        </span>
      </div>
      <div class="flex min-w-0 items-center gap-2">
        <Input
          id="egg-icon"
          type="file"
          accept="image/png,image/jpeg,image/webp"
          class="max-w-64"
          disabled={props.disabled}
          onchange={(e) => readIcon(e.currentTarget.files?.[0])}
        />
        {#if $formData.icon}
          <Button
            size="sm"
            variant="destructive"
            type="button"
            aria-label="Remove icon"
            disabled={props.disabled}
            onclick={() => ($formData.icon = '')}
          >
            Remove
          </Button>
        {/if}
      </div>
    </div>
  </div>
</div>

<div class="grid gap-2">
  <div class="grid gap-0.5">
    <span class="text-sm font-medium">Features</span>
    <span class="text-muted-foreground text-xs">
      Flags the panel uses to show extra prompts and checks for servers built
      from this egg.
    </span>
  </div>
  {#each KNOWN_FEATURES as feature (feature.key)}
    <Item.Root variant="outline" size="sm">
      <Item.Content>
        <Item.Title>
          {feature.label}
          <span class="text-muted-foreground font-mono text-xs font-normal">
            {feature.key}
          </span>
        </Item.Title>
        <Item.Description>{feature.description}</Item.Description>
      </Item.Content>
      <Item.Actions>
        <Switch
          aria-label={feature.label}
          disabled={props.disabled}
          bind:checked={$formData[feature.formKey]}
        />
      </Item.Actions>
    </Item.Root>
  {/each}
</div>

<FormTags
  {...props}
  key="customFeatures"
  label="Custom feature flags"
  placeholder="Type a flag and press Enter"
/>
