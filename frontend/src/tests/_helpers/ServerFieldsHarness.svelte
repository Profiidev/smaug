<script lang="ts">
  // Renders a server *Fields component inside a real BaseForm (superForm) so
  // tests can drive the inputs and read the bound form value back.
  import BaseForm from '@profidev/pleiades/components/form/base-form.svelte';
  import type { Component } from 'svelte';
  import ApplyErrors from '$lib/components/form/ApplyErrors.svelte';
  import {
    serverSettings,
    stageForm,
    type ServerForm
  } from '$routes/servers/schema.svelte';

  interface Props {
    fields: Component<{ formData: never; disabled: boolean }>;
    initialValue: ServerForm;
    disabled?: boolean;
    /** Extra props for the fields component (options, mode, egg, ...). */
    extra?: Record<string, unknown>;
    errors?: Record<string, string>;
    onsubmit?: (value: ServerForm) => void;
  }

  let {
    fields: Fields,
    initialValue,
    disabled = false,
    extra = {},
    errors,
    onsubmit = () => {}
  }: Props = $props();

  let form: BaseForm<typeof serverSettings> | undefined = $state();

  export const getValue = () => form?.getValue();
</script>

<BaseForm schema={serverSettings} {initialValue} {onsubmit} bind:this={form}>
  {#snippet children({ props })}
    <ApplyErrors formData={props.formData} {errors} />
    <Fields
      formData={stageForm(props.formData) as never}
      disabled={props.disabled || disabled}
      {...extra}
    />
  {/snippet}
  {#snippet footer()}
    <button type="submit">submit</button>
  {/snippet}
</BaseForm>
