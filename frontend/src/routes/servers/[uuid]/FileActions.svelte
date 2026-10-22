<script lang="ts">
  import Pencil from '@lucide/svelte/icons/pencil';
  import Download from '@lucide/svelte/icons/download';
  import EllipsisVertical from '@lucide/svelte/icons/ellipsis-vertical';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as DropdownMenu from '@profidev/pleiades/components/ui/dropdown-menu';
  import type { FileEntry } from '$lib/client';

  interface Props {
    entry: FileEntry;
    editHref?: string;
    downloadHref?: string;
    canWrite: boolean;
    onrename: (entry: FileEntry) => void;
    oncopy: (entry: FileEntry) => void;
    ondelete: (entry: FileEntry) => void;
  }

  let {
    entry,
    editHref,
    downloadHref,
    canWrite,
    onrename,
    oncopy,
    ondelete
  }: Props = $props();
</script>

<div class="ml-auto flex items-center justify-end gap-1">
  {#if editHref}
    <Button
      variant="ghost"
      size="icon-sm"
      href={editHref}
      aria-label={`Edit ${entry.name}`}
    >
      <Pencil />
    </Button>
  {/if}
  {#if downloadHref}
    <Button
      variant="ghost"
      size="icon-sm"
      href={downloadHref}
      download={entry.name}
      data-sveltekit-reload
      aria-label={`Download ${entry.name}`}
    >
      <Download />
    </Button>
  {/if}
  {#if canWrite}
    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <Button
            {...props}
            variant="ghost"
            size="icon-sm"
            class="cursor-pointer"
            aria-label={`Actions for ${entry.name}`}
          >
            <EllipsisVertical />
          </Button>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end">
        <DropdownMenu.Item onclick={() => onrename(entry)}>
          Rename
        </DropdownMenu.Item>
        <DropdownMenu.Item onclick={() => oncopy(entry)}>Copy</DropdownMenu.Item
        >
        <DropdownMenu.Separator />
        <DropdownMenu.Item
          variant="destructive"
          onclick={() => ondelete(entry)}
        >
          Delete
        </DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  {/if}
</div>
