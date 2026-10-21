<script lang="ts">
  import BaseForm from '@profidev/pleiades/components/form/base-form.svelte';
  import type {
    FormValue,
    SubmitHandler
  } from '@profidev/pleiades/components/form/types';
  import type { Snippet } from 'svelte';
  import ApplyErrors from '$lib/components/form/ApplyErrors.svelte';
  import {
    SECTION_DESCRIPTIONS,
    variablesEdited,
    type WizardData,
    generalStage
  } from './schema.svelte';
  import GeneralFields from './GeneralFields.svelte';

  interface Props {
    initialValue?: FormValue<typeof generalStage>;
    onsubmit: SubmitHandler<typeof generalStage>;
    footer: Snippet<[{ isLoading: boolean; isError: boolean }]>;
    isLoading?: boolean;
    data: WizardData;
  }

  let {
    initialValue,
    onsubmit,
    footer,
    isLoading = $bindable(false),
    data
  }: Props = $props();

  let form: BaseForm<typeof generalStage> | undefined = $state();
  // svelte-ignore state_referenced_locally
  const errors = data.takeErrors('general');

  export const getValue = () => form?.getValue();
</script>

<BaseForm
  schema={generalStage}
  {onsubmit}
  {footer}
  {initialValue}
  bind:this={form}
  bind:isLoading
  class="flex min-h-0 flex-col gap-4"
>
  {#snippet children({ props })}
    <ApplyErrors formData={props.formData} {errors} />
    <p class="text-muted-foreground -mt-4 text-sm">
      {SECTION_DESCRIPTIONS.general}
    </p>
    <!-- Plain block scroller: a sized grid/flex parent would squash overflow-hidden cards. -->
    <div class="-mx-1 min-h-0 overflow-y-auto px-1 pb-1">
      <div class="grid gap-4">
        <GeneralFields
          {...props}
          options={data.options}
          mode="create"
          variablesEdited={variablesEdited(data.values().variables)}
          oneggchange={data.oneggchange}
        />
      </div>
    </div>
  {/snippet}
</BaseForm>
