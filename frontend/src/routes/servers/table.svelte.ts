import { renderComponent } from '@tanstack/svelte-table';
import {
  type TableColumnDef,
  createColumnHeader
} from '@profidev/pleiades/components/table/helpers.svelte';
import Actions from '@profidev/pleiades/components/table/actions.svelte';
import type { Allocation, ServerListItem } from '$lib/client';
import MutedCell from '$lib/components/table/MutedCell.svelte';
import NameCell from './NameCell.svelte';
import StatusBadge from './StatusBadge.svelte';
import { availableActions } from './status';

/** List item plus flat strings the table sorts and searches on. */
export type ServerRow = ServerListItem & {
  allocation: string;
  egg_name: string;
  node_name: string;
};

/** `ip:port`, with IPv6 addresses bracketed. */
export const formatAllocation = ({ ip, port }: Allocation) =>
  ip.includes(':') ? `[${ip}]:${port}` : `${ip}:${port}`;

export const toRow = (server: ServerListItem): ServerRow => {
  const extra = server.extra_allocations.length;
  const primary = server.primary_allocation
    ? formatAllocation(server.primary_allocation)
    : '';
  return {
    ...server,
    allocation: extra ? `${primary} +${extra}` : primary,
    egg_name: server.egg.name,
    node_name: server.node.name
  };
};

const muted = (
  key: 'node_name' | 'allocation' | 'egg_name',
  title: string,
  cls?: string
) => ({
  ...createColumnHeader<ServerRow>(key, title),
  cell: ({ row }: { row: { original: ServerRow } }) =>
    renderComponent(MutedCell, {
      ...(cls ? { class: cls } : {}),
      text: row.original[key]
    })
});

export const columns = ({
  canEdit,
  deleteServer
}: {
  canEdit: boolean;
  deleteServer: (server: ServerRow) => void;
}): TableColumnDef<ServerRow>[] => [
  {
    ...createColumnHeader<ServerRow>('name', 'Name'),
    cell: ({ row }) =>
      renderComponent(NameCell, {
        href: `/servers/${row.original.uuid}`,
        name: row.original.name
      })
  },
  {
    ...createColumnHeader<ServerRow>('status', 'Status'),
    cell: ({ row }) =>
      renderComponent(StatusBadge, {
        class: 'ml-4',
        status: row.original.status
      })
  },
  muted('node_name', 'Node'),
  muted('allocation', 'Allocation', 'font-mono'),
  muted('egg_name', 'Egg'),
  {
    accessorKey: 'actions',
    cell: ({ row }) =>
      renderComponent(Actions, {
        delete_disabled:
          !canEdit || !availableActions(row.original.status).delete,
        // Owners and write grantees edit too; the edit page enforces access.
        edit: `/servers/${row.original.uuid}`,
        edit_disabled: false,
        remove: () => deleteServer(row.original)
      }),
    enableHiding: false,
    header: () => {}
  }
];
