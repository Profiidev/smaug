<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormSelect from '@profidev/pleiades/components/form/form-select.svelte';
  import FormTags from '@profidev/pleiades/components/form/form-tags.svelte';
  import { Badge } from '@profidev/pleiades/components/ui/badge';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Card from '@profidev/pleiades/components/ui/card';
  import * as Collapsible from '@profidev/pleiades/components/ui/collapsible';
  import * as Item from '@profidev/pleiades/components/ui/item';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { Switch } from '@profidev/pleiades/components/ui/switch';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Plus from '@lucide/svelte/icons/plus';
  import Trash from '@lucide/svelte/icons/trash';
  import FindEntry from './FindEntry.svelte';
  import { PARSER_OPTIONS } from './schema.svelte';
  import type {
    FormValue,
    SuperForm
  } from '@profidev/pleiades/components/form/types';
  import type { configStage } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof configStage>>;
    disabled: boolean;
  }

  let { ...props }: Props = $props();

  let formData = $derived(props.formData.form);

  const entryCount = (n: number) =>
    `${n} find ${n === 1 ? 'entry' : 'entries'}`;
</script>

<div class="grid gap-3">
  <div class="flex flex-wrap items-center justify-between gap-2">
    <div class="grid gap-0.5">
      <span class="text-sm font-medium">Config files</span>
      <span class="text-muted-foreground text-xs">
        Wings rewrites these files right before each start. With the
        <code>file</code> parser the key is a line prefix and the whole line is replaced.
      </span>
    </div>
    <Button
      size="sm"
      variant="outline"
      type="button"
      disabled={props.disabled}
      onclick={() =>
        ($formData.configFiles = [
          ...$formData.configFiles,
          { find: [], parser: ['file'], path: '' }
        ])}
    >
      <Plus />
      Add config file
    </Button>
  </div>

  {#each $formData.configFiles as file, i (i)}
    <Collapsible.Root open>
      {#snippet child({ props: rootProps })}
        <Card.Root {...rootProps} class="group/config gap-4 py-4">
          <Card.Header class="px-4">
            <Card.Title class="flex min-w-0 flex-wrap items-center gap-2">
              <Collapsible.Trigger>
                {#snippet child({ props: triggerProps })}
                  <Button
                    {...triggerProps}
                    size="icon-sm"
                    variant="ghost"
                    type="button"
                    aria-label="Toggle config file"
                  >
                    <ChevronDown
                      class="transition-transform group-data-[state=closed]/config:-rotate-90"
                    />
                  </Button>
                {/snippet}
              </Collapsible.Trigger>
              <span class="min-w-0 truncate font-mono text-sm">
                {file.path || 'New config file'}
              </span>
              <Badge variant="secondary">{file.parser[0]}</Badge>
              <span class="text-muted-foreground text-xs font-normal">
                {entryCount(file.find.length)}
              </span>
            </Card.Title>
            <Card.Action>
              <Button
                size="icon-sm"
                variant="destructive"
                type="button"
                aria-label="Remove config file"
                disabled={props.disabled}
                onclick={() =>
                  ($formData.configFiles = $formData.configFiles.filter(
                    (_, j) => j !== i
                  ))}
              >
                <Trash />
              </Button>
            </Card.Action>
          </Card.Header>
          <Collapsible.Content>
            <Card.Content class="grid gap-3 px-4">
              <div class="grid gap-4 md:grid-cols-2">
                <FormInput
                  {...props}
                  key={`configFiles[${i}].path`}
                  label="File Path"
                  class="font-mono"
                  placeholder="server.properties"
                />
                <FormSelect
                  {...props}
                  key={`configFiles[${i}].parser`}
                  label="Parser"
                  data={PARSER_OPTIONS}
                  single
                />
              </div>
              <div class="flex items-center justify-between gap-2">
                <span class="text-sm font-medium">Find entries</span>
                <Button
                  size="sm"
                  variant="ghost"
                  type="button"
                  disabled={props.disabled}
                  onclick={() =>
                    ($formData.configFiles[i].find = [
                      ...file.find,
                      { key: '', map: [], mode: ['string'], value: '' }
                    ])}
                >
                  <Plus />
                  Add find entry
                </Button>
              </div>
              {#each file.find as _, j (j)}
                <FindEntry
                  {...props}
                  {i}
                  {j}
                  onremove={() =>
                    ($formData.configFiles[i].find = file.find.filter(
                      (_, k) => k !== j
                    ))}
                />
              {/each}
            </Card.Content>
          </Collapsible.Content>
        </Card.Root>
      {/snippet}
    </Collapsible.Root>
  {/each}
</div>

<Separator />

<div class="grid gap-1">
  <FormTags
    {...props}
    key="fileDenylist"
    label="File Denylist"
    placeholder="*.env"
  />
  <p class="text-muted-foreground text-xs">
    Files users cannot open or change in the file manager.
  </p>
</div>

<div class="grid gap-2">
  <Item.Root variant="outline" size="sm">
    <Item.Content>
      <Item.Title>Custom logs</Item.Title>
      <Item.Description>
        The process writes its own log file. Ignored by Wings; kept for
        compatibility.
      </Item.Description>
    </Item.Content>
    <Item.Actions>
      <Switch
        aria-label="Custom logs"
        disabled={props.disabled}
        bind:checked={$formData.configLogsCustom}
      />
    </Item.Actions>
  </Item.Root>
  <div class="md:max-w-md">
    <FormInput
      {...props}
      key="configLogsLocation"
      label="Log File Location"
      class="font-mono"
      placeholder="logs/latest.log"
    />
  </div>
</div>
