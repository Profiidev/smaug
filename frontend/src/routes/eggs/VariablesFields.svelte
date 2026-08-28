<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormSelect from '@profidev/pleiades/components/form/form-select.svelte';
  import FormSwitch from '@profidev/pleiades/components/form/form-switch.svelte';
  import FormTextarea from './FormTextarea.svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Card } from '@profidev/pleiades/components/ui/card';
  import Plus from '@lucide/svelte/icons/plus';
  import Trash from '@lucide/svelte/icons/trash';
  import { FIELD_TYPE_OPTIONS } from './schema.svelte';
  import type { FormValue, SuperForm } from '@profidev/pleiades/components/form/types';
  import type { variablesStage } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof variablesStage>>;
    disabled: boolean;
  }

  let { ...props }: Props = $props();

  let formData = $derived(props.formData.form);

  const blankRow = () => ({
    defaultValue: '',
    description: '',
    envVariable: '',
    fieldType: ['text'] as ('text' | 'password')[],
    name: '',
    rules: '',
    userEditable: true,
    userViewable: true
  });

  const addVariable = () => {
    $formData.variables = [...$formData.variables, blankRow()];
  };

  const removeVariable = (index: number) => {
    $formData.variables = $formData.variables.filter((_, i) => i !== index);
  };
</script>

<div class="flex items-baseline justify-between">
  <span class="text-sm font-medium"
    >Startup variables ({$formData.variables.length})</span
  >
  <Button size="sm" onclick={addVariable} type="button">
    <Plus />
    Add variable
  </Button>
</div>

{#each $formData.variables as _, i (i)}
  <Card class="gap-3 p-4">
    <div class="flex items-center justify-between">
      <span class="text-muted-foreground text-sm font-medium">Variable {i + 1}</span>
      <Button
        size="icon-sm"
        variant="ghost"
        type="button"
        onclick={() => removeVariable(i)}
      >
        <Trash />
      </Button>
    </div>
    <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
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
        placeholder="SERVER_JARFILE"
      />
      <FormInput
        {...props}
        key={`variables[${i}].defaultValue`}
        label="Default Value"
        placeholder="server.jar"
      />
      <FormSelect
        {...props}
        key={`variables[${i}].fieldType`}
        label="Field Type"
        data={FIELD_TYPE_OPTIONS}
        single
      />
    </div>
    <FormInput
      {...props}
      key={`variables[${i}].rules`}
      label="Validation Rules"
      placeholder="required|string|max:20"
    />
    <FormTextarea
      {...props}
      key={`variables[${i}].description`}
      label="Description"
      rows={2}
      placeholder="Shown as help text under the field to end users."
    />
    <div class="flex gap-6">
      <FormSwitch {...props} key={`variables[${i}].userViewable`} label="Visible to users" />
      <FormSwitch {...props} key={`variables[${i}].userEditable`} label="Editable by users" />
    </div>
  </Card>
{/each}
