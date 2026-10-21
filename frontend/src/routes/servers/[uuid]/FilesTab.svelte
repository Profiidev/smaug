<script lang="ts">
  import { z } from 'zod';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import FilePlus from '@lucide/svelte/icons/file-plus';
  import FolderPlus from '@lucide/svelte/icons/folder-plus';
  import UploadIcon from '@lucide/svelte/icons/upload';
  import Search from '@lucide/svelte/icons/search';
  import Table from '@profidev/pleiades/components/table/clean-table.svelte';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Checkbox } from '@profidev/pleiades/components/ui/checkbox';
  import { Progress } from '@profidev/pleiades/components/ui/progress';
  import { Skeleton } from '@profidev/pleiades/components/ui/skeleton';
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import * as AlertDialog from '@profidev/pleiades/components/ui/alert-dialog';
  import * as Card from '@profidev/pleiades/components/ui/card';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
  import * as InputGroup from '@profidev/pleiades/components/ui/input-group';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import {
    cancelUpload,
    completeUpload,
    copyFile,
    createFile,
    createFolder,
    deleteFiles,
    listFiles,
    renameFile,
    startUpload,
    uploadChunk,
    type FileEntry,
    type ServerInfo
  } from '$lib/client';
  import FileActions from './FileActions.svelte';
  import FileEditor from './FileEditor.svelte';
  import FileNameCell from './FileNameCell.svelte';
  import { columns, sizeText } from './files-table.svelte';
  import {
    encodePath,
    formatBytes,
    isEditable,
    joinPath,
    normalize,
    relativeTime,
    resolvePath,
    ROOT_LABEL,
    segments,
    sendChunks,
    sortEntries
  } from './files';

  interface Props {
    server: ServerInfo;
    /** Live connection of the page shell; files do not use it. */
    live?: unknown;
  }

  let { server }: Props = $props();

  type Upload = {
    id: number;
    name: string;
    size: number;
    sent: number;
    started: number;
    status: 'uploading' | 'done' | 'failed' | 'cancelled';
    abort: AbortController;
    upload?: string;
  };

  const nameSchema = z.object({
    name: z.string().trim().min(1, 'Name is required')
  });

  let uuid = $derived(server.uuid);
  let canWrite = $derived(
    server.access === 'admin' || server.access === 'write'
  );
  let path = $derived(normalize(page.url.searchParams.get('path') ?? '/'));
  let edit = $derived(page.url.searchParams.get('edit'));
  let crumbs = $derived(segments(path));

  /** Undefined while loading. */
  let entries: FileEntry[] | undefined = $state.raw();
  let failed = $state(false);
  let search = $state('');
  let selected: string[] = $state.raw([]);
  let uploads: Upload[] = $state([]);
  let fileInput: HTMLInputElement | undefined = $state();

  let prompt: 'folder' | 'file' | 'rename' | undefined = $state();
  let promptOpen = $state(false);
  let promptLoading = $state(false);
  let renaming: FileEntry | undefined = $state.raw();
  let deleting: FileEntry[] = $state.raw([]);
  let deleteOpen = $state(false);
  let deleteLoading = $state(false);

  let filtered = $derived.by(() => {
    const term = search.trim().toLowerCase();
    return (entries ?? []).filter(
      (e) => !term || e.name.toLowerCase().includes(term)
    );
  });
  let allSelected = $derived(
    filtered.length > 0 && filtered.every((e) => selected.includes(e.name))
  );
  let totalSize = $derived(
    filtered.reduce((sum, e) => sum + (e.kind === 'dir' ? 0 : e.size), 0)
  );
  let uploading = $derived(uploads.some((u) => u.status === 'uploading'));

  const PROMPTS = {
    file: {
      confirm: 'Create',
      description: 'Creates an empty file and opens it in the editor.',
      title: 'New file'
    },
    folder: {
      confirm: 'Create',
      description: 'A path like `a/b` creates nested folders.',
      title: 'New folder'
    },
    rename: {
      confirm: 'Rename',
      description: 'A path like `../name` moves it to another folder.',
      title: 'Rename'
    }
  };

  const hrefFor = (dir: string, file?: string) =>
    `${page.url.pathname}?path=${encodePath(dir)}${file ? `&edit=${encodeURIComponent(file)}` : ''}`;
  const downloadHrefFor = (full: string) =>
    `/api/servers/${uuid}/files/download?path=${encodeURIComponent(full)}`;
  const dirOf = (full: string) => full.slice(0, full.lastIndexOf('/')) || '/';
  const baseOf = (full: string) => full.slice(full.lastIndexOf('/') + 1);

  let lastPath: string | undefined;
  let loadSeq = 0;
  const load = async () => {
    const seq = ++loadSeq;
    const target = path;
    const res = await listFiles({ path: { uuid }, query: { path: target } });
    if (seq !== loadSeq) return;
    failed = !res.data;
    entries = res.data ? sortEntries(res.data) : [];
  };

  // Reload on folder change and when the editor closes (sizes may differ).
  $effect(() => {
    if (edit) return;
    if (path !== lastPath) {
      lastPath = path;
      entries = undefined;
      selected = [];
      search = '';
    }
    load();
  });

  const retry = () => {
    entries = undefined;
    load();
  };

  const toggle = (name: string, on: boolean) => {
    selected = on
      ? [...selected.filter((n) => n !== name), name]
      : selected.filter((n) => n !== name);
  };
  const toggleAll = (on: boolean) => {
    selected = on ? filtered.map((e) => e.name) : [];
  };

  const openPrompt = (
    mode: 'folder' | 'file' | 'rename',
    entry?: FileEntry
  ) => {
    prompt = mode;
    renaming = entry;
    promptOpen = true;
  };

  const exists = (status?: number) =>
    status === 409
      ? ({
          error: 'Something with this name already exists',
          field: 'name'
        } as const)
      : ({ error: 'Request failed' } as const);

  const submitPrompt = async ({ name }: { name: string }) => {
    const target = resolvePath(path, name);
    if (prompt === 'folder') {
      const res = await createFolder({
        body: { path: target },
        path: { uuid }
      });
      if (!res.response?.ok) return exists(res.response?.status);
      toast.success(`Folder ${baseOf(target)} created`);
    } else if (prompt === 'file') {
      const res = await createFile({ body: { path: target }, path: { uuid } });
      if (!res.response?.ok) return exists(res.response?.status);
      promptOpen = false;
      goto(hrefFor(dirOf(target), baseOf(target)), {
        keepFocus: true,
        noScroll: true
      });
      return;
    } else if (renaming) {
      const from = joinPath(path, renaming.name);
      if (target === from) return;
      const res = await renameFile({
        body: { from, to: target },
        path: { uuid }
      });
      if (!res.response?.ok) return exists(res.response?.status);
      toast.success(`Renamed ${renaming.name}`);
      selected = selected.filter((n) => n !== renaming?.name);
    }
    load();
  };

  const copy = async (entry: FileEntry) => {
    const res = await copyFile({
      body: { path: joinPath(path, entry.name) },
      path: { uuid }
    });
    if (!res.data) {
      toast.error(`Failed to copy ${entry.name}`);
      return;
    }
    toast.success(`Copied to ${res.data.name}`);
    load();
  };

  const askDelete = (items: FileEntry[]) => {
    deleting = items;
    deleteOpen = true;
  };

  const confirmDelete = async () => {
    deleteLoading = true;
    const res = await deleteFiles({
      body: { paths: deleting.map((e) => joinPath(path, e.name)) },
      path: { uuid }
    });
    deleteLoading = false;
    if (!res.response?.ok) {
      toast.error('Failed to delete');
      return;
    }
    deleteOpen = false;
    toast.success(
      deleting.length === 1
        ? `Deleted ${deleting[0].name}`
        : `Deleted ${deleting.length} items`
    );
    const gone = new Set(deleting.map((e) => e.name));
    selected = selected.filter((n) => !gone.has(n));
    load();
  };

  let nextUpload = 0;

  const uploadOne = async (u: Upload, file: File, dir: string) => {
    const { signal } = u.abort;
    try {
      const start = await startUpload({
        body: { path: joinPath(dir, file.name), size: file.size },
        path: { uuid },
        signal
      });
      if (!start.data) throw new Error('start');
      const stream = start.data.upload;
      u.upload = stream;
      const sha256 = await sendChunks(
        file,
        start.data.chunk_size,
        async (index, chunk) => {
          const res = await uploadChunk({
            body: chunk,
            path: { index, stream, uuid },
            signal
          });
          if (!res.response?.ok) throw new Error('chunk');
          u.sent += chunk.length;
        }
      );
      const done = await completeUpload({
        body: { sha256 },
        path: { stream, uuid },
        signal
      });
      if (!done.response?.ok) throw new Error('complete');
      u.status = 'done';
    } catch {
      if (u.status !== 'uploading') return;
      u.status = 'failed';
      toast.error(`Failed to upload ${u.name}`);
      if (u.upload) cancelUpload({ path: { stream: u.upload, uuid } });
    }
  };

  const startUploads = async (files: File[]) => {
    const dir = path;
    const batch = files.map((file) => {
      const u: Upload = {
        abort: new AbortController(),
        id: nextUpload++,
        name: file.name,
        sent: 0,
        size: file.size,
        started: Date.now(),
        status: 'uploading'
      };
      uploads.push(u);
      // The pushed object is the reactive proxy; mutate that one.
      return { file, u: uploads[uploads.length - 1] };
    });
    await Promise.all(batch.map(({ file, u }) => uploadOne(u, file, dir)));
    // The file shows up once the node finished writing; poll twice.
    load();
    setTimeout(load, 1000);
    setTimeout(() => {
      if (!uploading) uploads = [];
    }, 3000);
  };

  const cancelAll = () => {
    for (const u of uploads) {
      if (u.status !== 'uploading') continue;
      u.status = 'cancelled';
      u.abort.abort();
      if (u.upload) cancelUpload({ path: { stream: u.upload, uuid } });
    }
    uploads = [];
  };

  const onfiles = (e: Event & { currentTarget: HTMLInputElement }) => {
    const files = [...(e.currentTarget.files ?? [])];
    e.currentTarget.value = '';
    if (files.length) startUploads(files);
  };

  const uploadStatus = (u: Upload) => {
    if (u.status === 'done') return 'Done';
    if (u.status === 'failed') return 'Failed';
    if (u.status === 'cancelled') return 'Cancelled';
    const percent = u.size ? Math.floor((u.sent / u.size) * 100) : 0;
    const seconds = Math.max((Date.now() - u.started) / 1000, 0.001);
    return `${percent}% · ${formatBytes(Math.round(u.sent / seconds))}/s`;
  };

  const onbeforeunload = (e: BeforeUnloadEvent) => {
    if (uploading) e.preventDefault();
  };

  let columnData = $derived({
    allSelected,
    canWrite,
    dirHref: (e: FileEntry) =>
      e.kind === 'dir' ? hrefFor(joinPath(path, e.name)) : undefined,
    downloadHref: (e: FileEntry) =>
      e.kind === 'dir' ? undefined : downloadHrefFor(joinPath(path, e.name)),
    editHref: (e: FileEntry) =>
      isEditable(e) ? hrefFor(path, e.name) : undefined,
    oncopy: copy,
    ondelete: (e: FileEntry) => askDelete([e]),
    onrename: (e: FileEntry) => openPrompt('rename', e),
    selected,
    toggle,
    toggleAll
  });
</script>

<svelte:window {onbeforeunload} />

{#if edit}
  <FileEditor
    {uuid}
    {canWrite}
    path={joinPath(path, edit)}
    backHref={hrefFor(path)}
    downloadHref={downloadHrefFor(joinPath(path, edit))}
  />
{:else}
  <div class="grid gap-3">
    <div class="flex flex-wrap items-center gap-2">
      <nav aria-label="Path" class="min-w-0 grow font-mono text-sm">
        <ol class="text-muted-foreground flex flex-wrap items-center gap-1">
          <li>
            {#if path === '/'}
              <span class="text-foreground" aria-current="page"
                >{ROOT_LABEL}</span
              >
            {:else}
              <a class="hover:text-foreground" href={hrefFor('/')}
                >{ROOT_LABEL}</a
              >
            {/if}
          </li>
          {#each crumbs as seg, i (seg.path)}
            <li aria-hidden="true">/</li>
            <li class="min-w-0 truncate">
              {#if i === crumbs.length - 1}
                <span class="text-foreground" aria-current="page"
                  >{seg.name}</span
                >
              {:else}
                <a class="hover:text-foreground" href={hrefFor(seg.path)}
                  >{seg.name}</a
                >
              {/if}
            </li>
          {/each}
        </ol>
      </nav>
      <InputGroup.Root class="w-full sm:w-56">
        <InputGroup.Addon><Search /></InputGroup.Addon>
        <InputGroup.Input
          bind:value={search}
          placeholder="Search files"
          aria-label="Search files"
        />
      </InputGroup.Root>
      {#if canWrite}
        <div class="flex flex-wrap gap-2">
          <Button
            variant="outline"
            class="cursor-pointer"
            onclick={() => openPrompt('file')}
          >
            <FilePlus />
            New file
          </Button>
          <Button
            variant="outline"
            class="cursor-pointer"
            onclick={() => openPrompt('folder')}
          >
            <FolderPlus />
            New folder
          </Button>
          <Button class="cursor-pointer" onclick={() => fileInput?.click()}>
            <UploadIcon />
            Upload
          </Button>
          <input
            bind:this={fileInput}
            type="file"
            multiple
            class="hidden"
            aria-label="Upload files"
            onchange={onfiles}
          />
        </div>
      {/if}
    </div>

    {#if uploads.length}
      <Card.Root>
        <Card.Header>
          <Card.Title>
            Uploading {uploads.length}
            {uploads.length === 1 ? 'file' : 'files'}
          </Card.Title>
          <Card.Description
            >Keep this tab open until the uploads finish.</Card.Description
          >
          {#if uploading}
            <Card.Action>
              <Button
                variant="outline"
                size="sm"
                class="cursor-pointer"
                onclick={cancelAll}
              >
                Cancel all
              </Button>
            </Card.Action>
          {/if}
        </Card.Header>
        <Card.Content class="grid gap-3">
          {#each uploads as u (u.id)}
            <div class="grid gap-1.5">
              <div class="flex items-center justify-between gap-2 text-sm">
                <span class="truncate font-mono">{u.name}</span>
                <span class="text-muted-foreground flex-none text-xs"
                  >{uploadStatus(u)}</span
                >
              </div>
              <Progress
                value={u.status === 'done'
                  ? 100
                  : u.size
                    ? (u.sent / u.size) * 100
                    : 0}
                aria-label={`Upload progress of ${u.name}`}
              />
            </div>
          {/each}
        </Card.Content>
      </Card.Root>
    {/if}

    {#if selected.length && canWrite}
      <div
        class="bg-muted/50 flex flex-wrap items-center gap-2 rounded-lg border px-3 py-2 text-sm"
      >
        <span class="mr-auto font-medium">
          {selected.length}
          {selected.length === 1 ? 'item' : 'items'} selected
        </span>
        <Button
          variant="destructive"
          size="sm"
          class="cursor-pointer"
          onclick={() =>
            askDelete((entries ?? []).filter((e) => selected.includes(e.name)))}
        >
          Delete
        </Button>
        <Button
          variant="ghost"
          size="sm"
          class="cursor-pointer"
          onclick={() => (selected = [])}
        >
          Clear
        </Button>
      </div>
    {/if}

    {#if failed}
      <Alert.Root variant="destructive">
        <Alert.Title>Failed to load files</Alert.Title>
        <Alert.Description>
          The node may be offline or the folder no longer exists.
          <Button
            variant="outline"
            size="sm"
            class="mt-2 cursor-pointer"
            onclick={retry}
          >
            Retry
          </Button>
        </Alert.Description>
      </Alert.Root>
    {:else if !entries}
      <div class="grid gap-3 rounded-md border p-4" aria-label="Loading files">
        {#each [1, 2, 3, 4, 5] as i (i)}
          <div class="flex items-center gap-3">
            <Skeleton class="size-7 rounded-lg" />
            <Skeleton class="h-4 grow" />
            <Skeleton class="h-4 w-16" />
          </div>
        {/each}
      </div>
    {:else if entries.length === 0}
      <Empty.Root class="border border-dashed">
        <Empty.Header>
          <Empty.Media variant="icon"><FolderOpen /></Empty.Media>
          <Empty.Title>This folder is empty</Empty.Title>
          <Empty.Description>
            {canWrite
              ? 'Upload files or create a new one.'
              : 'Nothing here yet.'}
          </Empty.Description>
        </Empty.Header>
        {#if canWrite}
          <Empty.Content class="flex-row justify-center">
            <Button class="cursor-pointer" onclick={() => fileInput?.click()}>
              <UploadIcon />
              Upload files
            </Button>
            <Button
              variant="outline"
              class="cursor-pointer"
              onclick={() => openPrompt('file')}
            >
              <FilePlus />
              New file
            </Button>
          </Empty.Content>
        {/if}
      </Empty.Root>
    {:else}
      <div class="hidden md:block">
        <Table data={filtered} {columns} {columnData} />
      </div>
      <ul class="grid gap-2 md:hidden" aria-label="Files">
        {#each filtered as entry (entry.name)}
          <li class="flex items-center gap-3 rounded-lg border px-3 py-2">
            {#if canWrite}
              <Checkbox
                aria-label={`Select ${entry.name}`}
                checked={selected.includes(entry.name)}
                onCheckedChange={(on) => toggle(entry.name, on)}
              />
            {/if}
            <div class="min-w-0 grow">
              <FileNameCell
                {entry}
                href={columnData.dirHref(entry)}
                detail={`${sizeText(entry)} · ${relativeTime(entry.modified)}`}
              />
            </div>
            <FileActions
              {entry}
              {canWrite}
              editHref={columnData.editHref(entry)}
              downloadHref={columnData.downloadHref(entry)}
              oncopy={copy}
              ondelete={columnData.ondelete}
              onrename={columnData.onrename}
            />
          </li>
        {:else}
          <li class="text-muted-foreground py-6 text-center text-sm">
            No results.
          </li>
        {/each}
      </ul>
      <span class="text-muted-foreground px-1 text-xs">
        {filtered.length}
        {filtered.length === 1 ? 'item' : 'items'} · {formatBytes(totalSize)}
      </span>
    {/if}
  </div>
{/if}

{#if prompt}
  <FormDialog
    title={PROMPTS[prompt].title}
    description={PROMPTS[prompt].description}
    confirm={PROMPTS[prompt].confirm}
    schema={nameSchema}
    initialValue={{
      name: renaming && prompt === 'rename' ? renaming.name : ''
    }}
    onsubmit={submitPrompt}
    bind:open={promptOpen}
    bind:isLoading={promptLoading}
  >
    {#snippet children({ props })}
      <FormInput
        {...props}
        key="name"
        label="Name"
        class="font-mono"
        autocomplete="off"
      />
    {/snippet}
  </FormDialog>
{/if}

<AlertDialog.Root bind:open={deleteOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>
        Delete {deleting.length}
        {deleting.length === 1 ? 'item' : 'items'}?
      </AlertDialog.Title>
      <AlertDialog.Description>
        This cannot be undone.
        {#if deleting.some((e) => e.kind === 'dir')}
          Folders are deleted with everything inside them.
        {/if}
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel class="cursor-pointer">Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        variant="destructive"
        class="cursor-pointer"
        disabled={deleteLoading}
        onclick={confirmDelete}
      >
        Delete
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
