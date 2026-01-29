<script lang="ts" generics="S extends FormRecord = FormRecord">
  import * as Form from 'positron-components/components/ui/form';
  import { type FormPath, type SuperForm } from 'sveltekit-superforms';
  import { Input } from 'positron-components/components/ui/input';
  import type {
    HTMLInputAttributes,
    HTMLInputTypeAttribute
  } from 'svelte/elements';
  import type { WithElementRef } from 'bits-ui';
  import type { FormRecord } from 'positron-components/components/form/types';
  import * as Tooltip from 'positron-components/components/ui/tooltip';
  import Info from '@lucide/svelte/icons/info';

  type InputType = Exclude<HTMLInputTypeAttribute, 'file'>;

  type InputProps = WithElementRef<
    Omit<HTMLInputAttributes, 'type'> &
      (
        | { type: 'file'; files?: FileList }
        | { type?: InputType; files?: undefined }
      )
  >;

  interface Props {
    formData: SuperForm<S>;
    key: FormPath<S>;
    label: string;
    disabled?: boolean;
    tooltip?: string;
  }

  let {
    formData: form,
    key,
    label,
    disabled,
    tooltip,
    ...restProps
  }: InputProps & Props = $props();

  let formData = $derived(form.form);
</script>

<Form.Field {form} name={key} class="gap-1/2 grid">
  <Form.Control>
    {#snippet children({ props })}
      <div class="flex gap-2">
        <Form.Label>{label}</Form.Label>
        {#if tooltip}
          <Tooltip.Provider>
            <Tooltip.Root>
              <Tooltip.Trigger>
                <Info class="size-4" />
              </Tooltip.Trigger>
              <Tooltip.Content>
                <p>{tooltip}</p>
              </Tooltip.Content>
            </Tooltip.Root>
          </Tooltip.Provider>
        {/if}
      </div>
      <Input {disabled} {...props} {...restProps} bind:value={$formData[key]} />
    {/snippet}
  </Form.Control>
  <Form.FieldErrors />
</Form.Field>
