<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import { Badge } from '@profidev/pleiades/components/ui/badge';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
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
</script>

{#if $formData.variables.length}
  <div class="grid gap-4 md:grid-cols-2">
    {#each $formData.variables as variable, i (variable.env)}
      <div class="grid content-start gap-2 rounded-xl border p-4">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <span class="text-sm font-medium">{variable.name}</span>
          <Badge variant="outline" class="font-mono">{variable.env}</Badge>
        </div>
        {#if variable.description}
          <p class="text-muted-foreground text-xs">{variable.description}</p>
        {/if}
        <FormInput
          {...props}
          key={`variables[${i}].value`}
          aria-label={variable.name}
          class="font-mono"
          placeholder={variable.defaultValue}
          disabled={props.disabled || !variable.editable}
        />
        {#if variable.rules.length}
          <p class="text-muted-foreground font-mono text-xs break-all">
            {variable.rules.join('|')}
          </p>
        {/if}
      </div>
    {/each}
  </div>
{:else}
  <Empty.Root class="border border-dashed">
    <Empty.Header>
      <Empty.Media variant="icon"><Variable /></Empty.Media>
      <Empty.Title>No variables</Empty.Title>
      <Empty.Description>This egg has no variables to set.</Empty.Description>
    </Empty.Header>
  </Empty.Root>
{/if}
