<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormTextarea from '@profidev/pleiades/components/form/form-textarea.svelte';
  import { Badge } from '@profidev/pleiades/components/ui/badge';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import * as Select from '@profidev/pleiades/components/ui/select';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { Switch } from '@profidev/pleiades/components/ui/switch';
  import * as Item from '@profidev/pleiades/components/ui/item';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import type {
    FormValue,
    SuperForm
  } from '@profidev/pleiades/components/form/types';
  import type { OptionEgg } from '$lib/client';
  import {
    defaultCommand,
    type FieldsMode,
    type startupStage
  } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof startupStage>>;
    disabled: boolean;
    /** The server's egg; its commands and images fill the selects. */
    egg?: OptionEgg;
    mode: FieldsMode;
    /** Locks the image (admin only on edit). */
    imageDisabled?: boolean;
  }

  let { egg, mode, imageDisabled = false, ...props }: Props = $props();

  let formData = $derived(props.formData.form);

  // bits-ui treats an empty value as "nothing selected", so Custom needs a real one.
  const CUSTOM = '__custom__';
  const id = $props.id();

  let commands = $derived(Object.entries(egg?.startup_commands ?? {}));
  let images = $derived(Object.entries(egg?.docker_images ?? {}));

  // The value held when Custom was picked; the select then stays on Custom
  // until the text changes (or the override is reset) instead of snapping
  // back to the matching egg entry.
  let customCommand: string | undefined = $state();
  let customImage: string | undefined = $state();

  let commandLabel = $derived(
    $formData.startupOverridden && customCommand === $formData.startup
      ? CUSTOM
      : (commands.find(([, cmd]) => cmd === $formData.startup)?.[0] ?? CUSTOM)
  );
  let imageLabel = $derived(
    customImage === $formData.image
      ? CUSTOM
      : (images.find(([, img]) => img === $formData.image)?.[0] ?? CUSTOM)
  );

  const pickCommand = (label: string) => {
    const cmd = egg?.startup_commands[label];
    customCommand = cmd === undefined ? $formData.startup : undefined;
    if (cmd === undefined) {
      $formData.startupOverridden = true;
      return;
    }
    $formData.startup = cmd;
    $formData.startupOverridden = cmd !== defaultCommand(egg);
  };

  const pickImage = (label: string) => {
    const img = egg?.docker_images[label];
    customImage = img === undefined ? $formData.image : undefined;
    if (img !== undefined) $formData.image = img;
  };

  const resetCommand = () => {
    $formData.startup = defaultCommand(egg);
    $formData.startupOverridden = false;
  };
</script>

{#if mode === 'create'}
  <div class="grid gap-2">
    <span class="text-sm font-medium">Installation</span>
    <Item.Root variant="outline" size="sm">
      <Item.Content>
        <Item.Title>Run install script</Item.Title>
        <Item.Description>
          Runs the egg's install script once. Turn off to skip it and mark the
          server as installed.
        </Item.Description>
      </Item.Content>
      <Item.Actions>
        <Switch
          aria-label="Run install script"
          disabled={props.disabled}
          bind:checked={$formData.runInstall}
        />
      </Item.Actions>
    </Item.Root>
    <Item.Root variant="outline" size="sm">
      <Item.Content>
        <Item.Title>Start after install</Item.Title>
        <Item.Description>
          Starts the server as soon as installation finishes.
        </Item.Description>
      </Item.Content>
      <Item.Actions>
        <Switch
          aria-label="Start after install"
          disabled={props.disabled}
          bind:checked={$formData.startOnCompletion}
        />
      </Item.Actions>
    </Item.Root>
  </div>
  <Separator />
{/if}

<div class="grid gap-2">
  <div class="flex flex-wrap items-center justify-between gap-2">
    <Label for="{id}-command">Startup command</Label>
    {#if $formData.startupOverridden}
      <div class="flex items-center gap-2">
        <Badge variant="outline">Overridden</Badge>
        <Button
          class="cursor-pointer"
          size="sm"
          variant="ghost"
          type="button"
          disabled={props.disabled || !egg}
          onclick={resetCommand}
        >
          <RotateCcw />
          Reset to default
        </Button>
      </div>
    {/if}
  </div>
  <Select.Root
    type="single"
    value={commandLabel}
    disabled={props.disabled}
    onValueChange={pickCommand}
  >
    <Select.Trigger id="{id}-command" class="w-full cursor-pointer">
      {commandLabel === CUSTOM ? 'Custom' : commandLabel}
    </Select.Trigger>
    <Select.Content>
      {#each commands as [label] (label)}
        <Select.Item value={label}>{label}</Select.Item>
      {/each}
      <Select.Item value={CUSTOM}>Custom</Select.Item>
    </Select.Content>
  </Select.Root>
  <FormTextarea
    {...props}
    key="startup"
    aria-label="Command"
    rows={3}
    class="font-mono"
    oninput={() => ($formData.startupOverridden = true)}
  />
  <p class="text-muted-foreground text-xs">
    {$formData.startupOverridden
      ? 'This server uses its own command. Egg updates will not change it.'
      : 'Follows the egg. Egg updates change it.'}
  </p>
</div>

<Separator />

<div class="grid gap-4 md:grid-cols-2">
  <div class="grid content-start gap-2">
    <Label for="{id}-image">Image name</Label>
    <Select.Root
      type="single"
      value={imageLabel}
      disabled={props.disabled || imageDisabled}
      onValueChange={pickImage}
    >
      <Select.Trigger id="{id}-image" class="w-full cursor-pointer">
        {imageLabel === CUSTOM ? 'Custom image' : imageLabel}
      </Select.Trigger>
      <Select.Content>
        {#each images as [label] (label)}
          <Select.Item value={label}>{label}</Select.Item>
        {/each}
        <Select.Item value={CUSTOM}>Custom image</Select.Item>
      </Select.Content>
    </Select.Root>
  </div>
  <div class="grid gap-1">
    <FormInput
      {...props}
      key="image"
      label="Image"
      disabled={props.disabled || imageDisabled}
      class="font-mono"
      placeholder="ghcr.io/pelican-eggs/yolks:java_21"
    />
    {#if imageLabel === CUSTOM}
      <p class="text-muted-foreground text-xs">
        Custom image. Must be pullable by the node.
      </p>
    {/if}
  </div>
</div>
