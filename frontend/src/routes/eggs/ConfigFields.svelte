<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormSelect from '@profidev/pleiades/components/form/form-select.svelte';
  import FormSwitch from '@profidev/pleiades/components/form/form-switch.svelte';
  import FormTags from '@profidev/pleiades/components/form/form-tags.svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Card } from '@profidev/pleiades/components/ui/card';
  import Plus from '@lucide/svelte/icons/plus';
  import Trash from '@lucide/svelte/icons/trash';
  import { PARSER_OPTIONS } from './schema.svelte';
  import type { FormValue, SuperForm } from '@profidev/pleiades/components/form/types';
  import type { configStage } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof configStage>>;
    disabled: boolean;
  }

  let { ...props }: Props = $props();

  let formData = $derived(props.formData.form);

  const blankFindEntry = () => ({
    isRegex: false,
    key: '',
    regexPattern: '',
    regexReplacement: '',
    value: ''
  });

  const addConfigFile = () => {
    $formData.configFiles = [
      ...$formData.configFiles,
      { find: [], parser: ['file'] as ('file' | 'yaml' | 'properties' | 'ini' | 'json' | 'xml')[], path: '' }
    ];
  };

  const removeConfigFile = (index: number) => {
    $formData.configFiles = $formData.configFiles.filter((_, i) => i !== index);
  };

  const addFindEntry = (fileIndex: number) => {
    $formData.configFiles = $formData.configFiles.map((file, i) =>
      i === fileIndex ? { ...file, find: [...file.find, blankFindEntry()] } : file
    );
  };

  const removeFindEntry = (fileIndex: number, entryIndex: number) => {
    $formData.configFiles = $formData.configFiles.map((file, i) =>
      i === fileIndex
        ? { ...file, find: file.find.filter((_, j) => j !== entryIndex) }
        : file
    );
  };
</script>

<div>
  <span class="text-sm font-medium">Config File Rewriting</span>
  <p class="text-muted-foreground mt-1 text-xs">
    Wings rewrites these paths inside the server files right before boot.
  </p>
  {#each $formData.configFiles as file, i (i)}
    <Card class="mt-2 gap-3 p-4">
      <div class="flex items-start gap-2">
        <div class="flex-[2]">
          <FormInput
            {...props}
            key={`configFiles[${i}].path`}
            label="File Path"
            placeholder="server.properties"
          />
        </div>
        <div class="flex-1">
          <FormSelect
            {...props}
            key={`configFiles[${i}].parser`}
            label="Parser"
            data={PARSER_OPTIONS}
            single
          />
        </div>
        <Button
          size="icon-sm"
          variant="ghost"
          type="button"
          class="mt-6"
          onclick={() => removeConfigFile(i)}
        >
          <Trash />
        </Button>
      </div>

      <span class="text-muted-foreground text-xs font-semibold uppercase"
        >Find / Replace</span
      >
      {#each file.find as entry, j (j)}
        <div class="bg-muted/40 flex items-start gap-2 rounded-md p-2">
          <div class="flex-1">
            <FormInput
              {...props}
              key={`configFiles[${i}].find[${j}].key`}
              label="Key"
              placeholder="server-port"
            />
          </div>
          <div class="w-28 pt-6">
            <FormSwitch
              {...props}
              key={`configFiles[${i}].find[${j}].isRegex`}
              label="Regex"
            />
          </div>
          {#if entry.isRegex}
            <div class="flex-1">
              <FormInput
                {...props}
                key={`configFiles[${i}].find[${j}].regexPattern`}
                label="Pattern"
                placeholder="^127\.0\.0\.1$"
              />
            </div>
            <div class="flex-1">
              <FormInput
                {...props}
                key={`configFiles[${i}].find[${j}].regexReplacement`}
                label="Replacement"
                placeholder={'{{server.build.default.ip}}'}
              />
            </div>
          {:else}
            <div class="flex-1">
              <FormInput
                {...props}
                key={`configFiles[${i}].find[${j}].value`}
                label="Value"
                placeholder={'{{server.build.default.port}}'}
              />
            </div>
          {/if}
          <Button
            size="icon-sm"
            variant="ghost"
            type="button"
            class="mt-6"
            onclick={() => removeFindEntry(i, j)}
          >
            <Trash />
          </Button>
        </div>
      {/each}
      <Button size="sm" variant="ghost" type="button" onclick={() => addFindEntry(i)}>
        <Plus />
        Add find entry
      </Button>
    </Card>
  {/each}
  <Button size="sm" variant="outline" type="button" class="mt-2" onclick={addConfigFile}>
    <Plus />
    Add config file
  </Button>
</div>

<FormTags
  {...props}
  key="fileDenylist"
  label="File Denylist"
  placeholder="*.env"
/>

<FormSwitch
  {...props}
  key="configLogsCustom"
  label="Process writes its own log file"
/>
<FormInput
  {...props}
  key="configLogsLocation"
  label="Log File Location"
  placeholder="logs/latest.log"
/>
