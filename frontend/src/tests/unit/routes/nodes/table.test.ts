import { describe, expect, it, vi } from 'vitest';
import Lock from '@lucide/svelte/icons/lock';
import LockOpen from '@lucide/svelte/icons/lock-open';
import MutedCell from '$lib/components/table/MutedCell.svelte';
import Status from '$lib/components/table/Status.svelte';
import NameCell from '$routes/servers/NameCell.svelte';
import { columns } from '$routes/nodes/table.svelte';
import type { NodeInfo } from '$lib/client';

vi.mock('@tanstack/svelte-table', async (orig) => ({
  ...(await orig<object>()),
  renderComponent: (component: unknown, props: unknown) => ({
    component,
    props
  })
}));

const node: NodeInfo = {
  address: 'wings.example.com',
  connected: true,
  id: 'n1',
  name: 'Alpha',
  port: 8443,
  secure: true
};

interface Cell {
  component: unknown;
  props: Record<string, unknown>;
}

const cellOf = (
  key: string,
  original: NodeInfo = node,
  args: Partial<Parameters<typeof columns>[0]> = {}
) => {
  const col = columns({ canEdit: true, deleteNode: vi.fn(), ...args }).find(
    (c) => 'accessorKey' in c && c.accessorKey === key
  )!;
  return (col.cell as (ctx: unknown) => Cell)({ row: { original } });
};

describe('nodes table columns', () => {
  it('shows name, status, address, secure, uuid and actions', () => {
    const keys = columns({ canEdit: true, deleteNode: vi.fn() }).map((c) =>
      'accessorKey' in c ? c.accessorKey : undefined
    );
    expect(keys).toEqual([
      'name',
      'connected',
      'address',
      'secure',
      'id',
      'actions'
    ]);
  });

  it('links the name to the node page', () => {
    const cell = cellOf('name');
    expect(cell.component).toBe(NameCell);
    expect(cell.props).toEqual({ href: '/nodes/n1/setup', name: 'Alpha' });
  });

  it.each([true, false])('renders the status for connected=%s', (connected) => {
    const cell = cellOf('connected', { ...node, connected });
    expect(cell.component).toBe(Status);
    expect(cell.props).toEqual({ connected });
  });

  it('renders the address with the port in mono', () => {
    const cell = cellOf('address');
    expect(cell.component).toBe(MutedCell);
    expect(cell.props).toEqual({
      class: 'font-mono',
      text: 'wings.example.com:8443'
    });
  });

  it('renders the uuid in mono', () => {
    const cell = cellOf('id');
    expect(cell.component).toBe(MutedCell);
    expect(cell.props).toEqual({ class: 'font-mono', text: 'n1' });
  });

  it('renders a green lock for secure nodes', () => {
    const cell = cellOf('secure');
    expect(cell.component).toBe(Lock);
    expect(cell.props.class).toContain('text-green-500');
    expect(cell.props['aria-label']).toBe('HTTPS');
  });

  it('renders an orange open lock for insecure nodes', () => {
    const cell = cellOf('secure', { ...node, secure: false });
    expect(cell.component).toBe(LockOpen);
    expect(cell.props.class).toContain('text-orange-500');
    expect(cell.props['aria-label']).toBe('HTTP');
  });

  it('links edit to the node page', () => {
    expect(cellOf('actions').props.edit).toBe('/nodes/n1/setup');
  });

  it.each([
    [true, false],
    [false, true]
  ])('canEdit=%s disables actions: %s', (canEdit, disabled) => {
    const { props } = cellOf('actions', node, { canEdit });
    expect(props.edit_disabled).toBe(disabled);
    expect(props.delete_disabled).toBe(disabled);
  });

  it('calls deleteNode with the row on remove', () => {
    const deleteNode = vi.fn();
    const { props } = cellOf('actions', node, { deleteNode });
    (props.remove as () => void)();
    expect(deleteNode).toHaveBeenCalledWith(node);
  });
});
