import type { NodeInfo } from '$lib/backend/node.svelte';
import Lock from '@lucide/svelte/icons/lock';
import LockOpen from '@lucide/svelte/icons/lock-open';
import type { ColumnDef } from '@tanstack/table-core';
import * as DataTable from '@profidev/pleiades/components/ui/data-table';
import {
  createColumn,
  createColumnHeader
} from '@profidev/pleiades/components/table/helpers.svelte';
import Actions from '$lib/components/table/Actions.svelte';
import { createRawSnippet } from 'svelte';
import Status from '$lib/components/table/Status.svelte';
import type { UserInfo } from '$lib/backend/user.svelte';
import { Permission } from '$lib/permissions.svelte';

export const columns = ({
  deleteNode,
  user
}: {
  deleteNode: (node: NodeInfo) => void;
  user?: UserInfo;
}): ColumnDef<NodeInfo>[] => [
  {
    accessorKey: 'connected',
    cell: ({ row }) => {
      const connected = row.getValue<boolean>('connected');
      return DataTable.renderComponent(Status, {
        connected
      });
    },
    header: () => {}
  },
  createColumn('name', 'Name'),
  {
    ...createColumnHeader('address', 'Address'),
    cell: ({ row }) => {
      const address = row.getValue<string>('address');
      const {port} = row.original;
      const value = `${address}:${port}`;

      return DataTable.renderSnippet(
        createRawSnippet(() => ({
            render: () =>
              `<div class="ml-4 truncate h-full w-full text-wrap">${value}</div>`
          }))
      );
    }
  },
  {
    ...createColumnHeader('secure', 'Secure'),
    cell: ({ row }) => {
      const secure = row.getValue<boolean>('secure');
      return DataTable.renderComponent(secure ? Lock : LockOpen, {
        class: `ml-3 ${secure ? 'text-green-500' : 'text-orange-500'}`
      });
    }
  },
  createColumn('id', 'UUID'),
  {
    accessorKey: 'actions',
    cell: ({ row }) => DataTable.renderComponent(Actions, {
        delete_disabled: !user?.permissions.includes(Permission.NODE_EDIT),
        editHref: `/nodes/${row.original.id}/setup`,
        edit_disabled: !user?.permissions.includes(Permission.NODE_EDIT),
        remove: () => deleteNode(row.original)
      }),
    enableHiding: false,
    header: () => {}
  }
];
