<script lang="ts" generics="S extends FormRecord = FormRecord">
  import * as Form from 'positron-components/components/ui/form';
  import { type FormPath, type SuperForm } from 'sveltekit-superforms';
  import * as Password from 'positron-components/components/ui-extra/password';
  import type { FormRecord } from 'positron-components/components/form/types';

  interface Props {
    formData: SuperForm<S>;
    key: FormPath<S>;
    label: string;
    disabled?: boolean;
    placeholder?: string;
  }

  let { formData: form, key, label, disabled, placeholder }: Props = $props();

  let formData = $derived(form.form);
</script>

<Form.Field {form} name={key} class="gap-1/2 grid">
  <Form.Control>
    {#snippet children({ props })}
      <Form.Label>{label}</Form.Label>
      <Password.Root>
        {/* @ts-ignore */ null}
        <Password.Input
          {disabled}
          {placeholder}
          {...props}
          bind:value={$formData[key]}
        >
          <Password.ToggleVisibility />
        </Password.Input>
      </Password.Root>
    {/snippet}
  </Form.Control>
  <Form.FieldErrors />
</Form.Field>
