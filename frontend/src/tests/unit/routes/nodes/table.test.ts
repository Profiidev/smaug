import { describe, expect, it, vi } from 'vitest';
import Lock from '@lucide/svelte/icons/lock';
import LockOpen from '@lucide/svelte/icons/lock-open';
import Status from '$lib/components/table/Status.svelte';
import { columns } from '$routes/nodes/table.svelte';
import { Permission } from '$lib/permissions.svelte';
import type { NodeInfo, UserInfo } from '$lib/client';

vi.mock('@tanstack/svelte-table', async (orig) => ({
  ...(await orig<object>()),
  renderComponent: (component: unknown, props: unknown) => ({
    component,
    props
  }),
  renderSnippet: (snippet: unknown) => ({ snippet })
}));
// Hand back the raw render fn so the address cell markup can be inspected.
vi.mock('svelte', async (orig) => ({
  ...(await orig<object>()),
  createRawSnippet: (fn: unknown) => fn
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
  component?: unknown;
  props: Record<string, unknown>;
  snippet?: () => { render: () => string };
}

const cellOf = (
  key: string,
  original: NodeInfo = node,
  args: Partial<Parameters<typeof columns>[0]> = {}
) => {
  const col = columns({ deleteNode: vi.fn(), ...args }).find(
    (c) => 'accessorKey' in c && c.accessorKey === key
  )!;
  return (col.cell as (ctx: unknown) => Cell)({
    row: {
      getValue: (k: keyof NodeInfo) => original[k],
      original
    }
  });
};

const userWith = (...permissions: Permission[]) =>
  ({ permissions }) as unknown as UserInfo;

describe('nodes table columns', () => {
  it('shows status, name, address, secure, uuid and actions', () => {
    const keys = columns({ deleteNode: vi.fn() }).map((c) =>
      'accessorKey' in c ? c.accessorKey : undefined
    );
    expect(keys).toEqual([
      'connected',
      'name',
      'address',
      'secure',
      'id',
      'actions'
    ]);
  });

  it.each([true, false])('renders the status for connected=%s', (connected) => {
    const cell = cellOf('connected', { ...node, connected });
    expect(cell.component).toBe(Status);
    expect(cell.props).toEqual({ connected });
  });

  it('renders the address together with the port', () => {
    const cell = cellOf('address');
    expect(cell.snippet?.().render()).toContain('wings.example.com:8443');
  });

  it('renders a green lock for secure nodes', () => {
    const cell = cellOf('secure');
    expect(cell.component).toBe(Lock);
    expect(cell.props.class).toContain('text-green-500');
  });

  it('renders an orange open lock for insecure nodes', () => {
    const cell = cellOf('secure', { ...node, secure: false });
    expect(cell.component).toBe(LockOpen);
    expect(cell.props.class).toContain('text-orange-500');
  });

  it('links edit to the node setup page', () => {
    const { props } = cellOf('actions', node, {
      user: userWith(Permission.NODE_EDIT)
    });
    expect(props.edit).toBe('/nodes/n1/setup');
  });

  it.each([
    ['no user', undefined, true],
    ['view only', userWith(Permission.NODE_VIEW), true],
    ['node:edit', userWith(Permission.NODE_EDIT), false]
  ])('actions disabled for %s: %s', (_name, user, disabled) => {
    const { props } = cellOf('actions', node, { user });
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
