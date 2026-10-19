<script lang="ts">
  import { untrack } from 'svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Spinner } from '@profidev/pleiades/components/ui/spinner';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import Save from '@lucide/svelte/icons/save';
  import type { SuperForm } from '@profidev/pleiades/components/form/types';
  import type { EggForm } from './schema.svelte';

  interface Props {
    formData: SuperForm<EggForm>;
    /** Last loaded or saved value; becomes the form's clean (untainted) state. */
    saved?: EggForm;
    isLoading: boolean;
    isError: boolean;
    readonly: boolean;
  }

  let { formData, saved, isLoading, isError, readonly }: Props = $props();

  let tainted = $derived(formData.tainted);
  let dirty = $derived(formData.isTainted($tainted));

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
