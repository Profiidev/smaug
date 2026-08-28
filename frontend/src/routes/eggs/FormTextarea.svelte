<script lang="ts" generics="S extends FormRecord = FormRecord">
  import * as Form from '@profidev/pleiades/components/ui/form';
  import { type FormPath, type SuperForm } from 'sveltekit-superforms';
  import { Textarea } from '@profidev/pleiades/components/ui/textarea';
  import type { HTMLTextareaAttributes } from 'svelte/elements';
  import type { FormRecord } from '@profidev/pleiades/components/form/types';

  interface Props {
    formData: SuperForm<S>;
    key: FormPath<S>;
    label: string;
    disabled?: boolean;
  }

  let {
    formData: form,
    key,
    label,
    disabled,
    ...restProps
  }: HTMLTextareaAttributes & Props = $props();

  let formData = $derived(form.form);
</script>

<Form.Field {form} name={key} class="gap-1/2 grid">
  <Form.Control>
    {#snippet children({ props })}
      <Form.Label>{label}</Form.Label>
      {/* @ts-ignore */ null}
      <Textarea {disabled} {...props} {...restProps} bind:value={$formData[key]} />
    {/snippet}
  </Form.Control>
  <Form.FieldErrors />
</Form.Field>
