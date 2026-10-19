<script lang="ts" generics="S extends FormRecord = FormRecord">
  // FormInput without a visible label: for list rows whose columns are labelled once above.
  import {
    fieldProxy,
    type FormPath,
    type SuperForm
  } from 'sveltekit-superforms';
  import * as Form from '@profidev/pleiades/components/ui/form';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import type { FormRecord } from '@profidev/pleiades/components/form/types';

  interface Props {
    formData: SuperForm<S>;
    key: FormPath<S>;
    /** Accessible name, since there is no visible label. */
    label: string;
    disabled?: boolean;
    placeholder?: string;
    class?: string;
  }

  let {
    formData: form,
    key,
    label,
    disabled,
    placeholder,
    class: className
  }: Props = $props();

  let field: any = $derived(fieldProxy(form, key));
</script>

<Form.Field {form} name={key} class="grid gap-1">
  <Form.Control>
    {#snippet children({ props })}
      <Input
        {...props}
        {disabled}
        {placeholder}
        class={className}
        aria-label={label}
        bind:value={$field}
      />
    {/snippet}
  </Form.Control>
  <Form.FieldErrors />
</Form.Field>
