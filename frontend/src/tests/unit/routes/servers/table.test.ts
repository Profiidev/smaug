import { describe, expect, it, vi } from 'vitest';
import {
  type ServerRow,
  columns,
  formatAllocation,
  toRow
} from '$routes/servers/table.svelte';
import MutedCell from '$lib/components/table/MutedCell.svelte';
import Container from '@lucide/svelte/icons/container';
import NameCell from '$routes/servers/NameCell.svelte';
import StatusBadge from '$routes/servers/StatusBadge.svelte';
import { servers } from './fixtures';

vi.mock('@tanstack/svelte-table', async (orig) => ({
  ...(await orig<object>()),
  renderComponent: (component: unknown, props: unknown) => ({
    component,
    props
  })
}));

interface Cell {
  component: unknown;
  props: Record<string, unknown>;
}

const row = toRow(servers[0]);

const cellOf = (key: string, r: ServerRow = row, canEdit = true) => {
  const deleteServer = vi.fn();
  const col = columns({ canEdit, deleteServer }).find(
    (c) => 'accessorKey' in c && c.accessorKey === key
  )!;
  return {
    cell: (col.cell as (ctx: unknown) => Cell)({ row: { original: r } }),
    deleteServer
  };
};

describe('formatAllocation / toRow', () => {
  it('brackets IPv6 addresses', () => {
    expect(formatAllocation({ ip: '10.0.0.5', port: 1 })).toBe('10.0.0.5:1');
    expect(formatAllocation({ ip: '::1', port: 2 })).toBe('[::1]:2');
  });

  it('flattens node, egg and allocation with the extra count', () => {
    expect(row).toMatchObject({
      allocation: '10.0.0.5:25565 +1',
      egg_name: 'Paper',
      node_name: 'eu-1'
    });
    expect(toRow(servers[1]).allocation).toBe('[::1]:30000');
  });

  it('handles a server without a primary allocation', () => {
    expect(toRow({ ...servers[1], primary_allocation: null }).allocation).toBe(
      ''
    );
  });
});

describe('servers table columns', () => {
  it('shows name, status, node, allocation, egg and actions', () => {
    const keys = columns({ canEdit: true, deleteServer: vi.fn() }).map((c) =>
      'accessorKey' in c ? c.accessorKey : undefined
    );
    expect(keys).toEqual([
      'name',
      'status',
      'node_name',
      'allocation',
      'egg_name',
      'actions'
    ]);
  });

  it('renders the name as an icon + link to the server', () => {
    expect(cellOf('name').cell).toEqual({
      component: NameCell,
      props: {
        href: '/servers/srv-1/status',
        icon: Container,
        name: 'survival'
      }
    });
  });

  it('renders the status badge', () => {
    expect(cellOf('status').cell).toEqual({
      component: StatusBadge,
      props: { class: 'ml-4', status: 'offline' }
    });
  });

  it.each([
    ['node_name', { text: 'eu-1' }],
    ['allocation', { class: 'font-mono', text: '10.0.0.5:25565 +1' }],
    ['egg_name', { text: 'Paper' }]
  ])('renders %s muted', (key, props) => {
    expect(cellOf(key).cell).toEqual({ component: MutedCell, props });
  });

  // Owners and write grantees may edit without server:edit; the page enforces access.
  it.each([true, false])('always enables edit (canEdit=%s)', (canEdit) => {
    const { props } = cellOf('actions', row, canEdit).cell;
    expect(props.edit).toBe('/servers/srv-1/status');
    expect(props.edit_disabled).toBe(false);
  });

  it.each([
    [true, 'offline', false],
    [true, 'suspended', false],
    [true, 'installing', true],
    [false, 'offline', true],
    [false, 'installing', true]
  ] as const)(
    'delete with canEdit=%s and status %s is disabled=%s',
    (canEdit, status, disabled) => {
      const { props } = cellOf('actions', { ...row, status }, canEdit).cell;
      expect(props.delete_disabled).toBe(disabled);
    }
  );

  it('calls deleteServer with the row', () => {
    const { cell, deleteServer } = cellOf('actions');
    (cell.props.remove as () => void)();
    expect(deleteServer).toHaveBeenCalledWith(row);
  });
});
