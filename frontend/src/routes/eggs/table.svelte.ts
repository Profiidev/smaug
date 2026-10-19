import { renderComponent } from '@tanstack/svelte-table';
import {
  type TableColumnDef,
  createColumn
} from '@profidev/pleiades/components/table/helpers.svelte';
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
}): TableColumnDef<EggListItem>[] => [
  createColumn('name', 'Name'),
  createColumn('author', 'Author'),
  createColumn('description', 'Description', (value: string | null) => value ?? ''),
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
