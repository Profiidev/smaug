<script lang="ts" generics="S extends FormRecord = FormRecord">
  import { Checkbox } from '@profidev/pleiades/components/ui/checkbox';
  import * as Form from '@profidev/pleiades/components/ui/form';
  import type {
    FormPath,
    SuperForm,
    FormRecord
  } from '@profidev/pleiades/components/form/types';
  import type { WithoutChildrenOrChild } from '@profidev/pleiades/utils';
  import { Checkbox as CheckboxPrimitive } from 'bits-ui';

  interface Props {
    formData: SuperForm<S>;
    key: FormPath<S>;
    label: string;
    disabled?: boolean;
    switchOrder?: boolean;
  }

  let {
    formData: form,
    key,
    label,
    disabled,
    switchOrder,
    ...restProps
  }: Props & WithoutChildrenOrChild<CheckboxPrimitive.RootProps> = $props();

  let formData: any = $derived(form.form);
</script>

<Form.Field {form} name={key} class="mt-2 flex w-full flex-col">
  <Form.Control>
    {#snippet children({ props })}
      {#if switchOrder}
        <div class="flex">
          <Checkbox
            {...props}
            {...restProps}
            bind:checked={$formData[key]}
            class="mr-2"
          />
          <Form.Label>{label}</Form.Label>
        </div>
      {:else}
        <div class="flex">
          <Form.Label>{label}</Form.Label>
          <Checkbox
            {...props}
            {...restProps}
            bind:checked={$formData[key]}
            class="ml-auto"
          />
        </div>
      {/if}
    {/snippet}
  </Form.Control>
  <Form.FieldErrors />
</Form.Field>
