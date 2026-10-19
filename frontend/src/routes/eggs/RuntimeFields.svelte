<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormTags from '@profidev/pleiades/components/form/form-tags.svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { Switch } from '@profidev/pleiades/components/ui/switch';
  import * as Item from '@profidev/pleiades/components/ui/item';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import Plus from '@lucide/svelte/icons/plus';
  import X from '@lucide/svelte/icons/x';
  import type {
    FormValue,
    SuperForm
  } from '@profidev/pleiades/components/form/types';
  import type { runtimeStage } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof runtimeStage>>;
    disabled: boolean;
  }

  let { ...props }: Props = $props();

  let formData = $derived(props.formData.form);

  // Wings matches `done` verbatim, so keep whitespace; only reject empties/duplicates.
  const keepVerbatim = (val: string, tags: string[]) =>
    val && !tags.includes(val) ? val : undefined;
</script>

{#snippet heading(
  title: string,
  caption: string,
  add: string,
  onadd: () => void
)}
  <div class="flex flex-wrap items-center justify-between gap-2">
    <div class="grid gap-0.5">
      <span class="text-sm font-medium">{title}</span>
      <span class="text-muted-foreground text-xs">{caption}</span>
    </div>
    <Button
      size="sm"
      variant="outline"
      type="button"
      disabled={props.disabled}
      onclick={onadd}
    >
      <Plus />
      {add}
    </Button>
  </div>
{/snippet}

{#snippet columns(left: string, right: string)}
  <div
    class="flex flex-wrap gap-2 text-sm font-medium sm:flex-nowrap"
    aria-hidden="true"
  >
    <span class="min-w-0 flex-[1_1_140px]">{left}</span>
    <span class="hidden w-4 flex-none sm:block"></span>
    <span class="min-w-0 flex-[3_1_220px]">{right}</span>
    <span class="w-8 flex-none"></span>
  </div>
{/snippet}

{#snippet removeButton(label: string, onclick: () => void)}
  <Button
    size="icon-sm"
    variant="destructive"
    type="button"
    class="mt-0.5"
    aria-label={label}
    disabled={props.disabled}
    {onclick}
  >
    <X />
  </Button>
{/snippet}

<div class="grid gap-2">
  {@render heading(
    'Docker Images',
    'Users pick one label when they create a server. At least one image is required.',
    'Add image',
    () =>
      ($formData.dockerImages = [
        ...$formData.dockerImages,
        { image: '', label: '' }
      ])
  )}
  {#if $formData.dockerImages.length}
    {@render columns('Label', 'Image')}
  {/if}
  {#each $formData.dockerImages as _, i (i)}
    <div class="flex flex-wrap items-start gap-2 sm:flex-nowrap">
      <div class="min-w-0 flex-[1_1_140px]">
        <FormInput
          {...props}
          key={`dockerImages[${i}].label`}
          aria-label="Label"
          placeholder="Java 21"
        />
      </div>
      <ArrowRight
        class="text-muted-foreground mt-2 hidden size-4 flex-none sm:block"
      />
      <div class="min-w-0 flex-[3_1_220px]">
        <FormInput
          {...props}
          key={`dockerImages[${i}].image`}
          aria-label="Image"
          class="font-mono"
          placeholder="ghcr.io/pelican-eggs/yolks:java_21"
        />
      </div>
      {@render removeButton(
        'Remove image',
        () =>
          ($formData.dockerImages = $formData.dockerImages.filter(
            (_, j) => j !== i
          ))
      )}
    </div>
  {/each}
</div>

<Separator />

<div class="grid gap-2">
  {@render heading(
    'Startup Commands',
    'Label → command. Commands must be distinct. The first one is used for Pterodactyl exports.',
    'Add startup command',
    () =>
      ($formData.startupCommands = [
        ...$formData.startupCommands,
        { command: '', label: '' }
      ])
  )}
  {#if $formData.startupCommands.length}
    {@render columns('Label', 'Command')}
  {/if}
  {#each $formData.startupCommands as _, i (i)}
    <div class="flex flex-wrap items-start gap-2 sm:flex-nowrap">
      <div class="min-w-0 flex-[1_1_140px]">
        <FormInput
          {...props}
          key={`startupCommands[${i}].label`}
          aria-label="Label"
          placeholder="Default"
        />
      </div>
      <ArrowRight
        class="text-muted-foreground mt-2 hidden size-4 flex-none sm:block"
      />
      <div class="min-w-0 flex-[3_1_220px]">
        <FormInput
          {...props}
          key={`startupCommands[${i}].command`}
          aria-label="Command"
          class="font-mono"
          placeholder={'java -Xms128M -Xmx{{SERVER_MEMORY}}M -jar {{SERVER_JARFILE}}'}
        />
      </div>
      {@render removeButton(
        'Remove startup command',
        () =>
          ($formData.startupCommands = $formData.startupCommands.filter(
            (_, j) => j !== i
          ))
      )}
    </div>
  {/each}
</div>

<Separator />

<div class="grid gap-1">
  <FormTags
    {...props}
    key="configStartupDone"
    label="Done String(s)"
    validate={keepVerbatim}
    placeholder={'Substring, or regex:<pattern> — press Enter after each'}
  />
  <p class="text-muted-foreground text-xs">
    The server counts as started when the console prints any of these. Prefix
    with regex: for a regular expression.
  </p>
</div>

<div class="grid gap-1 md:max-w-md">
  <FormInput
    {...props}
    key="configStop"
    label="Stop Command"
    class="font-mono"
    placeholder="stop, or ^C for SIGINT"
  />
  <p class="text-muted-foreground text-xs">
    A value like ^C sends the SIGINT signal instead of typing a command.
  </p>
</div>

<Item.Root variant="outline" size="sm">
  <Item.Content>
    <Item.Title>Strip ANSI codes</Item.Title>
    <Item.Description>
      Remove colour and cursor escape sequences from console output before
      matching the done strings.
    </Item.Description>
  </Item.Content>
  <Item.Actions>
    <Switch
      aria-label="Strip ANSI codes"
      disabled={props.disabled}
      bind:checked={$formData.configStripAnsi}
    />
  </Item.Actions>
</Item.Root>

<div class="grid gap-1">
  <FormTags
    {...props}
    key="configUserInteraction"
    label="User interaction prompts"
    placeholder="Do you agree?"
  />
  <p class="text-muted-foreground text-xs">
    Console prompts that ask the user to answer, such as the EULA question.
    Ignored by Wings; kept for compatibility.
  </p>
</div>
