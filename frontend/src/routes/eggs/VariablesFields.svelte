<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormSwitch from '@profidev/pleiades/components/form/form-switch.svelte';
  import FormTags from '@profidev/pleiades/components/form/form-tags.svelte';
  import FormTextarea from '@profidev/pleiades/components/form/form-textarea.svelte';
  import { Badge } from '@profidev/pleiades/components/ui/badge';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Card from '@profidev/pleiades/components/ui/card';
  import * as Collapsible from '@profidev/pleiades/components/ui/collapsible';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import Plus from '@lucide/svelte/icons/plus';
  import Trash from '@lucide/svelte/icons/trash';
  import Variable from '@lucide/svelte/icons/variable';
  import type {
    FormValue,
    SuperForm
  } from '@profidev/pleiades/components/form/types';
  import type { variablesStage } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof variablesStage>>;
    disabled: boolean;
  }

  let { ...props }: Props = $props();

  let formData = $derived(props.formData.form);
  let errors = $derived(props.formData.errors);

  // Loaded variables start collapsed; added ones open (see addVariable).
  let open: boolean[] = $state([]);

  // Never hide errors: expand every variable that has a field error.
  $effect(() => {
    const errs = ($errors as { variables?: Record<number, unknown> }).variables;
    if (!errs) return;
    $formData.variables.forEach((_, i) => {
      if (errs[i]) open[i] = true;
    });
  });

  const addVariable = () => {
    $formData.variables = [
      ...$formData.variables,
      {
        defaultValue: '',
        description: '',
        envVariable: '',
        name: '',
        rules: ['required', 'string'],
        userEditable: false,
        userViewable: false
      }
    ];
    open[$formData.variables.length - 1] = true;
  };

  const removeVariable = (index: number) => {
    $formData.variables = $formData.variables.filter((_, i) => i !== index);
    open = open.filter((_, i) => i !== index);
  };
</script>

<div class="flex flex-wrap items-center justify-between gap-2">
  <span class="text-sm font-medium">
    Startup variables ({$formData.variables.length})
  </span>
  <Button
    class="cursor-pointer"
    size="sm"
    variant="outline"
    onclick={addVariable}
    type="button"
    disabled={props.disabled}
  >
    <Plus />
    Add variable
  </Button>
</div>

{#each $formData.variables as variable, i (i)}
  <Collapsible.Root bind:open={() => !!open[i], (v) => (open[i] = v)}>
    {#snippet child({ props: rootProps })}
      <Card.Root {...rootProps} class="group/variable gap-4 py-4">
        <Card.Header class="px-4">
          <Card.Title class="flex min-w-0 flex-wrap items-center gap-2">
            <Collapsible.Trigger>
              {#snippet child({ props: triggerProps })}
                <Button
                  class="cursor-pointer"
                  {...triggerProps}
                  size="icon-sm"
                  variant="ghost"
                  type="button"
                  aria-label="Toggle variable"
                >
                  <ChevronDown
                    class="transition-transform group-data-[state=closed]/variable:-rotate-90"
                  />
                </Button>
              {/snippet}
            </Collapsible.Trigger>
            <Badge variant="outline">#{i + 1}</Badge>
            <span class="min-w-0 truncate"
              >{variable.name || 'New variable'}</span
            >
            {#if variable.envVariable}
              <Badge variant="secondary" class="font-mono">
                {variable.envVariable}
              </Badge>
            {/if}
          </Card.Title>
          <Card.Action>
            <Button
              class="cursor-pointer"
              size="icon-sm"
              variant="destructive"
              type="button"
              aria-label="Remove variable"
              disabled={props.disabled}
              onclick={() => removeVariable(i)}
            >
              <Trash />
            </Button>
          </Card.Action>
        </Card.Header>
        <Collapsible.Content>
          <Card.Content class="grid gap-4 px-4">
            <div class="grid gap-4 md:grid-cols-2">
              <FormInput
                {...props}
                key={`variables[${i}].name`}
                label="Name"
                placeholder="Server Jar File"
              />
              <FormInput
                {...props}
                key={`variables[${i}].envVariable`}
                label="Env Variable"
                class="font-mono"
                placeholder="SERVER_JARFILE"
              />
              <FormInput
                {...props}
                key={`variables[${i}].defaultValue`}
                label="Default Value"
                class="font-mono"
                placeholder="server.jar"
              />
            </div>
            <FormTags
              {...props}
              key={`variables[${i}].rules`}
              label="Validation Rules"
              placeholder="required, string, max:20 — press Enter after each"
            />
            <FormTextarea
              {...props}
              key={`variables[${i}].description`}
              label="Description"
              rows={2}
              placeholder="Shown as help text under the field to end users."
            />
            <div class="grid gap-x-6 sm:grid-cols-2">
              <FormSwitch
                {...props}
                key={`variables[${i}].userViewable`}
                label="Visible to users"
              />
              <FormSwitch
                {...props}
                key={`variables[${i}].userEditable`}
                label="Editable by users"
              />
            </div>
          </Card.Content>
        </Collapsible.Content>
      </Card.Root>
    {/snippet}
  </Collapsible.Root>
{:else}
  <Empty.Root class="border border-dashed">
    <Empty.Header>
      <Empty.Media variant="icon"><Variable /></Empty.Media>
      <Empty.Title>No variables yet</Empty.Title>
      <Empty.Description>
        Variables let users change values such as the game version or memory
        when they create a server.
      </Empty.Description>
    </Empty.Header>
  </Empty.Root>
{/each}
