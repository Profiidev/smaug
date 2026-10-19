import { renderComponent } from '@tanstack/svelte-table';
import {
  type TableColumnDef,
  createColumnHeader
} from '@profidev/pleiades/components/table/helpers.svelte';
import Actions from '@profidev/pleiades/components/table/actions.svelte';
import type { EggListItem } from '$lib/client';
import MutedCell from './MutedCell.svelte';
import NameCell from './NameCell.svelte';

export const columns = ({
  canEdit,
  deleteEgg
}: {
  canEdit: boolean;
  deleteEgg: (egg: EggListItem) => void;
}): TableColumnDef<EggListItem>[] => [
  {
    ...createColumnHeader<EggListItem>('name', 'Name'),
    cell: ({ row }) =>
      renderComponent(NameCell, {
        href: `/eggs/${row.original.uuid}`,
        name: row.original.name
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
