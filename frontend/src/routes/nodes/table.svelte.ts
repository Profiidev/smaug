import { renderComponent } from '@tanstack/svelte-table';
import Lock from '@lucide/svelte/icons/lock';
import LockOpen from '@lucide/svelte/icons/lock-open';
import {
  type TableColumnDef,
  createColumnHeader
} from '@profidev/pleiades/components/table/helpers.svelte';
import Actions from '@profidev/pleiades/components/table/actions.svelte';
import MutedCell from '$lib/components/table/MutedCell.svelte';
import Status from '$lib/components/table/Status.svelte';
import type { NodeInfo, Runtime } from '$lib/client';
import NameCell from '../servers/NameCell.svelte';

const RUNTIME_LABELS: Record<Runtime, string> = {
  docker: 'Docker',
  kubernetes: 'Kubernetes'
};

export const columns = ({
  canEdit,
  deleteNode
}: {
  canEdit: boolean;
  deleteNode: (node: NodeInfo) => void;
}): TableColumnDef<NodeInfo>[] => [
  {
    ...createColumnHeader<NodeInfo>('name', 'Name'),
    cell: ({ row }) =>
      renderComponent(NameCell, {
        href: `/nodes/${row.original.id}/setup`,
        name: row.original.name
      })
  },
  {
    ...createColumnHeader<NodeInfo>('connected', 'Status'),
    cell: ({ row }) =>
      renderComponent(Status, { connected: row.original.connected })
  },
  {
    ...createColumnHeader<NodeInfo>('runtime', 'Runtime'),
    cell: ({ row }) => {
      const { runtime } = row.original;
      return renderComponent(MutedCell, {
        text: runtime ? RUNTIME_LABELS[runtime] : '—'
      });
    }
  },
  {
    ...createColumnHeader<NodeInfo>('address', 'Address'),
    cell: ({ row }) =>
      renderComponent(MutedCell, {
        class: 'font-mono',
        text: `${row.original.address}:${row.original.port}`
      })
  },
  {
    ...createColumnHeader<NodeInfo>('secure', 'Secure'),
    cell: ({ row }) => {
      const { secure } = row.original;
      return renderComponent(secure ? Lock : LockOpen, {
        'aria-label': secure ? 'HTTPS' : 'HTTP',
        class: `ml-4 ${secure ? 'text-green-500' : 'text-orange-500'}`
      });
    }
  },
  {
    ...createColumnHeader<NodeInfo>('id', 'UUID'),
    cell: ({ row }) =>
      renderComponent(MutedCell, { class: 'font-mono', text: row.original.id })
  },
  {
    accessorKey: 'actions',
    cell: ({ row }) =>
      renderComponent(Actions, {
        delete_disabled: !canEdit,
        edit: `/nodes/${row.original.id}/setup`,
        edit_disabled: !canEdit,
        remove: () => deleteNode(row.original)
      }),
    enableHiding: false,
    header: () => {}
  }
];
