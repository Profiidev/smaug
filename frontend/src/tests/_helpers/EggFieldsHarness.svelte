<script lang="ts">
  // Renders an egg *Fields component inside a real BaseForm (superForm) so
  // tests can drive the inputs and read the bound form value back.
  import BaseForm from '@profidev/pleiades/components/form/base-form.svelte';
  import type { Component } from 'svelte';
  import {
    eggSettings,
    stageForm,
    type EggForm
  } from '$routes/eggs/schema.svelte';

  interface Props {
    fields: Component<{ formData: never; disabled: boolean }>;
    initialValue: EggForm;
    disabled?: boolean;
    onsubmit?: (value: EggForm) => void;
  }

  let {
    fields: Fields,
    initialValue,
    disabled = false,
    onsubmit = () => {}
  }: Props = $props();

  let form: BaseForm<typeof eggSettings> | undefined = $state();

  export const getValue = () => form?.getValue();
</script>

<BaseForm schema={eggSettings} {initialValue} {onsubmit} bind:this={form}>
  {#snippet children({ props })}
    <Fields
      formData={stageForm(props.formData) as never}
      disabled={props.disabled || disabled}
    />
  {/snippet}
  {#snippet footer()}
    <button type="submit">submit</button>
  {/snippet}
</BaseForm>
