import type { NodeInfo } from '$lib/backend/node.svelte';
import Lock from '@lucide/svelte/icons/lock';
import LockOpen from '@lucide/svelte/icons/lock-open';
import type { ColumnDef } from '@tanstack/table-core';
import * as DataTable from 'positron-components/components/ui/data-table';
import {
  createColumn,
  createColumnHeader
} from 'positron-components/components/table/helpers.svelte';
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
    header: () => {},
    cell: ({ row }) => {
      let connected = row.getValue<boolean>('connected');
      return DataTable.renderComponent(Status, {
        connected
      });
    }
  },
  createColumn('name', 'Name'),
  {
    ...createColumnHeader('address', 'Address'),
    cell: ({ row }) => {
      let address = row.getValue<string>('address');
      let port = row.original.port;
      let value = `${address}:${port}`;

      return DataTable.renderSnippet(
        createRawSnippet(() => {
          return {
            render: () =>
              `<div class="ml-4 truncate h-full w-full text-wrap">${value}</div>`
          };
        })
      );
    }
  },
  {
    ...createColumnHeader('secure', 'Secure'),
    cell: ({ row }) => {
      let secure = row.getValue<boolean>('secure');
      return DataTable.renderComponent(secure ? Lock : LockOpen, {
        class: `ml-3 ${secure ? 'text-green-500' : 'text-orange-500'}`
      });
    }
  },
  createColumn('id', 'UUID'),
  {
    accessorKey: 'actions',
    header: () => {},
    cell: ({ row }) => {
      return DataTable.renderComponent(Actions, {
        edit_disabled: !user?.permissions.includes(Permission.NODE_EDIT),
        delete_disabled: !user?.permissions.includes(Permission.NODE_EDIT),
        editHref: `/nodes/${row.original.id}`,
        remove: () => deleteNode(row.original)
      });
    },
    enableHiding: false
  }
];
