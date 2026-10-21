<script lang="ts" generics="T extends FormRecord">
  import { untrack } from 'svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Spinner } from '@profidev/pleiades/components/ui/spinner';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import Save from '@lucide/svelte/icons/save';
  import type {
    FormRecord,
    SuperForm
  } from '@profidev/pleiades/components/form/types';

  interface Props {
    formData: SuperForm<T>;
    /** Last loaded or saved value; becomes the form's clean (untainted) state. */
    saved?: T;
    isLoading: boolean;
    isError: boolean;
    readonly: boolean;
    /** Out: whether the form has unsaved changes. */
    dirty?: boolean;
  }

  let {
    formData,
    saved,
    isLoading,
    isError,
    readonly,
    dirty = $bindable(false)
  }: Props = $props();

  let tainted = $derived(formData.tainted);

  $effect.pre(() => {
    dirty = formData.isTainted($tainted);
  });

  $effect(() => {
    if (!saved) return;
    const newState = saved;
    untrack(() => formData.reset({ newState }));
  });
</script>

<div
  class="bg-background sticky bottom-0 z-10 mt-auto flex flex-wrap items-center justify-end gap-3 border-t py-3"
>
  {#if dirty && !readonly}
    <span class="text-muted-foreground mr-auto text-sm">Unsaved changes</span>
  {/if}
  <Button
    class="cursor-pointer"
    type="submit"
    disabled={isLoading || readonly}
    variant={isError ? 'destructive' : undefined}
  >
    {#if isLoading}
      <Spinner />
    {:else if isError}
      <RotateCcw />
    {:else}
      <Save />
    {/if}
    {isError ? 'Retry' : 'Save Changes'}
  </Button>
</div>
