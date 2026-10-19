<script lang="ts">
  import * as Dialog from '@profidev/pleiades/components/ui/dialog';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Checkbox } from '@profidev/pleiades/components/ui/checkbox';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import { Badge } from '@profidev/pleiades/components/ui/badge';
  import { Spinner } from '@profidev/pleiades/components/ui/spinner';
  import CloudDownload from '@lucide/svelte/icons/cloud-download';
  import { toast } from '@profidev/pleiades/components/util/general';
  import type { EggForm } from './schema.svelte';
  import {
    applyEggUpdate,
    lineDiff,
    UPDATE_SECTIONS,
    type EggUpdate,
    type SectionKey
  } from './update';

  interface Props {
    open: boolean;
    update?: EggUpdate;
    onapplied?: (egg: EggForm) => void;
  }

  let { open = $bindable(), update, onapplied }: Props = $props();

  let selected: SectionKey[] = $derived([...(update?.changed ?? [])]);
  let applying = $state(false);

  let diff = $derived(
    update?.changed.includes('install')
      ? lineDiff(update.current.scriptsScript, update.source.scriptsScript)
      : []
  );
  let added = $derived(diff.filter((l) => l.kind === 'add').length);
  let removed = $derived(diff.filter((l) => l.kind === 'del').length);

  const summary = (key: SectionKey) => {
    if (!update?.changed.includes(key)) return 'Same as the installed version.';
    if (key === 'install' && diff.length) {
      return `+${added} ${added === 1 ? 'line' : 'lines'}, −${removed} ${removed === 1 ? 'line' : 'lines'}.`;
    }
    return 'Differs from the installed version.';
  };

  const toggle = (key: SectionKey, on: boolean) => {
    selected = on ? [...selected, key] : selected.filter((k) => k !== key);
  };

  const apply = async () => {
    if (!update) return;
    applying = true;
    try {
      const egg = await applyEggUpdate(update, selected);
      toast.success(`Egg ${egg.name} updated`, {
        description: 'The selected sections now match the source.'
      });
      open = false;
      onapplied?.(egg);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : 'Failed to update egg');
    } finally {
      applying = false;
    }
  };
</script>

<Dialog.Root
  bind:open={
    () => open,
    (v) => {
      if (!applying) open = v;
    }
  }
>
  <Dialog.Content class="max-h-[90vh] overflow-y-auto sm:max-w-xl">
    <Dialog.Header>
      <Dialog.Title>Update {update?.current.name}</Dialog.Title>
      <Dialog.Description>
        Choose which sections to replace with the source version.
      </Dialog.Description>
    </Dialog.Header>
    <div class="grid gap-2">
      {#each UPDATE_SECTIONS as section (section.key)}
        {@const same = !update?.changed.includes(section.key)}
        <div class="flex items-start gap-3 rounded-lg border px-3 py-2.5">
          <Checkbox
            id="upd-{section.key}"
            class="mt-0.5"
            checked={selected.includes(section.key)}
            disabled={same || applying}
            onCheckedChange={(v) => toggle(section.key, v)}
          />
          <div class="grid min-w-0 flex-1 gap-0.5">
            <Label for="upd-{section.key}">{section.title}</Label>
            <span class="text-muted-foreground text-xs">
              {summary(section.key)}
            </span>
          </div>
          {#if same}
            <Badge variant="secondary">No changes</Badge>
          {/if}
        </div>
      {/each}
    </div>
    {#if diff.length}
      <div class="grid gap-1.5">
        <span class="text-muted-foreground text-xs">
          Install script, changes from source
        </span>
        <div
          class="bg-muted max-h-64 overflow-auto rounded-lg border py-1 font-mono text-xs"
        >
          {#each diff as line, i (i)}
            <div
              class={[
                'flex gap-2 px-2.5 leading-5 break-all whitespace-pre-wrap',
                line.kind === 'add' && 'bg-green-500/15',
                line.kind === 'del' && 'bg-red-500/15',
                line.kind === 'ctx' && 'text-muted-foreground'
              ]}
            >
              <span class="w-3 flex-none">
                {line.kind === 'add' ? '+' : line.kind === 'del' ? '−' : ' '}
              </span>
              <span>{line.text}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}
    <Dialog.Footer>
      <Button
        variant="outline"
        class="cursor-pointer"
        disabled={applying}
        onclick={() => (open = false)}
      >
        Cancel
      </Button>
      <Button
        class="cursor-pointer"
        disabled={applying || selected.length === 0}
        onclick={apply}
      >
        {#if applying}
          <Spinner />
          Updating…
        {:else}
          <CloudDownload />
          Update {selected.length}
          {selected.length === 1 ? 'section' : 'sections'}
        {/if}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
