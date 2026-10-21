<script lang="ts" generics="T extends FormRecord">
  import type {
    FormRecord,
    SuperForm
  } from '@profidev/pleiades/components/form/types';

  interface Props {
    formData: SuperForm<T>;
    keys: string[];
  }

  let { formData, keys }: Props = $props();

  let errors = $derived(formData.errors);
  let hasError = $derived(
    keys.some((k) => ($errors as Record<string, unknown>)[k] !== undefined)
  );
</script>

{#if hasError}
  <span
    class="bg-destructive size-2 rounded-full"
    role="img"
    aria-label="Has errors"
  ></span>
{/if}
