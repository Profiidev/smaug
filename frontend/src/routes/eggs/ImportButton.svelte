<script lang="ts">
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { toast } from '@profidev/pleiades/components/util/general';
  import Upload from '@lucide/svelte/icons/upload';
  import { readEggFile, type EggImport } from './schema.svelte';

  interface Props {
    onimport: (egg: EggImport, fileName: string) => void;
    disabled?: boolean;
  }

  let { onimport, disabled }: Props = $props();

  let input: HTMLInputElement | undefined = $state();

  const onchange = async (e: Event & { currentTarget: HTMLInputElement }) => {
    const target = e.currentTarget;
    const file = target.files?.[0];
    if (!file) return;
    try {
      onimport(await readEggFile(file), file.name);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Invalid egg file');
    }
    target.value = '';
  };
</script>

<input
  bind:this={input}
  type="file"
  accept="application/json,.json"
  aria-label="Import egg JSON (Pelican or Pterodactyl)"
  class="sr-only"
  tabindex="-1"
  {disabled}
  {onchange}
/>
<Button
  variant="outline"
  type="button"
  class="cursor-pointer"
  {disabled}
  onclick={() => input?.click()}
>
  <Upload />
  Import JSON
</Button>
