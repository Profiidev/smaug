import type { ColumnDef } from '@tanstack/table-core';
import * as DataTable from '@profidev/pleiades/components/ui/data-table';
import { createColumn } from '@profidev/pleiades/components/table/helpers.svelte';
import Actions from '@profidev/pleiades/components/table/actions.svelte';

// TODO: replace with the generated $lib/client EggListItem/EggInfo type once backend egg routes exist.
export interface EggListItem {
  author: string;
  description: string | null;
  name: string;
  uuid: string;
}

export const columns = ({
  canEdit,
  deleteEgg
}: {
  canEdit: boolean;
  deleteEgg: (egg: EggListItem) => void;
}): ColumnDef<EggListItem>[] => [
  createColumn('name', 'Name'),
  createColumn('author', 'Author'),
  createColumn('description', 'Description', (value: string | null) => value ?? ''),
  {
    accessorKey: 'actions',
    cell: ({ row }) =>
      DataTable.renderComponent(Actions, {
        delete_disabled: !canEdit,
        edit: `/eggs/${row.original.uuid}`,
        edit_disabled: !canEdit,
        remove: () => deleteEgg(row.original)
      }),
    enableHiding: false,
    header: () => {}
  }
];
