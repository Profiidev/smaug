import { renderComponent } from '@tanstack/svelte-table';
import {
  type TableColumnDef,
  createColumnHeader
} from '@profidev/pleiades/components/table/helpers.svelte';
import Actions from '@profidev/pleiades/components/table/actions.svelte';
import type { EggListItem } from '$lib/client';
import type { EggUpdate } from './update';
import MutedCell from './MutedCell.svelte';
import NameCell from './NameCell.svelte';

export const columns = ({
  canEdit,
  deleteEgg,
  updates = {},
  failed = {},
  review
}: {
  canEdit: boolean;
  deleteEgg: (egg: EggListItem) => void;
  updates?: Record<string, EggUpdate>;
  failed?: Record<string, string>;
  review?: (update: EggUpdate) => void;
}): TableColumnDef<EggListItem>[] => [
  {
    ...createColumnHeader<EggListItem>('name', 'Name'),
    cell: ({ row }) =>
      renderComponent(NameCell, {
        failed: failed[row.original.uuid],
        href: `/eggs/${row.original.uuid}`,
        name: row.original.name,
        onreview:
          updates[row.original.uuid] && review
            ? () => review(updates[row.original.uuid])
            : undefined
      })
  },
  {
    ...createColumnHeader<EggListItem>('author', 'Author'),
    cell: ({ row }) => renderComponent(MutedCell, { text: row.original.author })
  },
  {
    ...createColumnHeader<EggListItem>('description', 'Description'),
    cell: ({ row }) =>
      renderComponent(MutedCell, {
        class: 'block max-w-md truncate',
        text: row.original.description
      })
  },
  {
    accessorKey: 'actions',
    cell: ({ row }) =>
      renderComponent(Actions, {
        delete_disabled: !canEdit,
        edit: `/eggs/${row.original.uuid}`,
        edit_disabled: !canEdit,
        remove: () => deleteEgg(row.original)
      }),
    enableHiding: false,
    header: () => {}
  }
];
