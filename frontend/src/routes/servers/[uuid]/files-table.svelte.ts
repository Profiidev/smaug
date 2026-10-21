import { renderComponent } from '@tanstack/svelte-table';
import type { TableColumnDef } from '@profidev/pleiades/components/table/helpers.svelte';
import { Checkbox } from '@profidev/pleiades/components/ui/checkbox';
import type { FileEntry } from '$lib/client';
import MutedCell from '$lib/components/table/MutedCell.svelte';
import FileActions from './FileActions.svelte';
import FileNameCell from './FileNameCell.svelte';
import { formatBytes, relativeTime } from './files';

export interface FileColumnData {
  canWrite: boolean;
  /** Names of the selected entries. */
  selected: string[];
  /** All visible entries are selected. */
  allSelected: boolean;
  toggle: (name: string, on: boolean) => void;
  toggleAll: (on: boolean) => void;
  dirHref: (entry: FileEntry) => string | undefined;
  editHref: (entry: FileEntry) => string | undefined;
  downloadHref: (entry: FileEntry) => string | undefined;
  onrename: (entry: FileEntry) => void;
  oncopy: (entry: FileEntry) => void;
  ondelete: (entry: FileEntry) => void;
}

export const sizeText = (entry: FileEntry) =>
  entry.kind === 'dir' ? '—' : formatBytes(entry.size);

// Plain string headers: the list stays folders first, sorted by name.
export const columns = (d: FileColumnData): TableColumnDef<FileEntry>[] => [
  ...(d.canWrite
    ? [
        {
          accessorKey: 'select',
          cell: ({ row }) =>
            renderComponent(Checkbox, {
              'aria-label': `Select ${row.original.name}`,
              checked: d.selected.includes(row.original.name),
              class: 'ml-2',
              onCheckedChange: (on: boolean) => d.toggle(row.original.name, on)
            }),
          header: () =>
            renderComponent(Checkbox, {
              'aria-label': 'Select all',
              checked: d.allSelected,
              class: 'ml-2',
              indeterminate: !d.allSelected && d.selected.length > 0,
              onCheckedChange: (on: boolean) => d.toggleAll(on)
            })
        } satisfies TableColumnDef<FileEntry>
      ]
    : []),
  {
    accessorKey: 'name',
    cell: ({ row }) =>
      renderComponent(FileNameCell, {
        entry: row.original,
        href: d.dirHref(row.original)
      }),
    header: 'Name'
  },
  {
    accessorKey: 'size',
    cell: ({ row }) =>
      renderComponent(MutedCell, {
        class: 'ml-0 font-mono',
        text: sizeText(row.original)
      }),
    header: 'Size'
  },
  {
    accessorKey: 'modified',
    cell: ({ row }) =>
      renderComponent(MutedCell, {
        class: 'ml-0',
        text: relativeTime(row.original.modified),
        title: row.original.modified.toLocaleString()
      }),
    header: 'Modified'
  },
  {
    accessorKey: 'actions',
    cell: ({ row }) =>
      renderComponent(FileActions, {
        canWrite: d.canWrite,
        downloadHref: d.downloadHref(row.original),
        editHref: d.editHref(row.original),
        entry: row.original,
        oncopy: d.oncopy,
        ondelete: d.ondelete,
        onrename: d.onrename
      }),
    enableHiding: false,
    header: 'Actions'
  }
];
