<script lang="ts">
  import FileIcon from '@lucide/svelte/icons/file';
  import Folder from '@lucide/svelte/icons/folder';
  import FileSymlink from '@lucide/svelte/icons/file-symlink';
  import type { FileEntry } from '$lib/client';

  interface Props {
    entry: FileEntry;
    /** Folders open via this link. */
    href?: string;
    /** Shown under the name (mobile cards). */
    detail?: string;
  }

  let { entry, href, detail }: Props = $props();

  let Icon = $derived(
    entry.kind === 'dir'
      ? Folder
      : entry.kind === 'symlink'
        ? FileSymlink
        : FileIcon
  );
</script>

<div class="flex min-w-0 items-center gap-2.5">
  <div
    class="bg-muted text-muted-foreground flex size-7 flex-none items-center justify-center rounded-lg"
  >
    <Icon class="size-4" />
  </div>
  <div class="grid min-w-0">
    {#if href}
      <a
        {href}
        class="truncate font-mono underline-offset-4 hover:underline"
        data-sveltekit-noscroll
        data-sveltekit-keepfocus>{entry.name}</a
      >
    {:else}
      <span class="truncate font-mono">{entry.name}</span>
    {/if}
    {#if detail}
      <span class="text-muted-foreground truncate text-xs">{detail}</span>
    {/if}
  </div>
</div>
