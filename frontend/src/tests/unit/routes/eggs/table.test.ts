import { describe, expect, it, vi } from 'vitest';
import { columns } from '$routes/eggs/table.svelte';
import type { EggListItem } from '$lib/client';
import MutedCell from '$routes/eggs/MutedCell.svelte';
import NameCell from '$routes/eggs/NameCell.svelte';

vi.mock('@tanstack/svelte-table', async (orig) => ({
  ...(await orig<object>()),
  renderComponent: (component: unknown, props: unknown) => ({
    component,
    props
  })
}));

const egg: EggListItem = {
  author: 'a@b.co',
  description: null,
  has_update_url: false,
  name: 'Paper',
  uuid: 'egg-1'
};

interface Cell {
  props: Record<string, unknown>;
}
const actions = (canEdit: boolean, deleteEgg = vi.fn()) => {
  const col = columns({ canEdit, deleteEgg }).find(
    (c) => 'accessorKey' in c && c.accessorKey === 'actions'
  )!;
  const cell = (col.cell as (ctx: unknown) => Cell)({ row: { original: egg } });
  return { deleteEgg, props: cell.props };
};

describe('eggs table columns', () => {
  it('shows name, author, description and actions', () => {
    const keys = columns({ canEdit: true, deleteEgg: vi.fn() }).map((c) =>
      'accessorKey' in c ? c.accessorKey : undefined
    );
    expect(keys).toEqual(['name', 'author', 'description', 'actions']);
  });

  const cellOf = (key: string, row: EggListItem = egg) => {
    const col = columns({ canEdit: true, deleteEgg: vi.fn() }).find(
      (c) => 'accessorKey' in c && c.accessorKey === key
    )!;
    return (col.cell as (ctx: unknown) => Cell & { component: unknown })({
      row: { original: row }
    });
  };

  it('renders the name as an icon + link to the egg', () => {
    expect(cellOf('name')).toEqual({
      component: NameCell,
      props: { href: '/eggs/egg-1', name: 'Paper' }
    });
  });

  it('renders the author muted', () => {
    expect(cellOf('author')).toEqual({
      component: MutedCell,
      props: { text: 'a@b.co' }
    });
  });

  it.each([null, 'A long description'])(
    'renders the description (%s) muted and truncated',
    (description) => {
      expect(cellOf('description', { ...egg, description })).toEqual({
        component: MutedCell,
        props: { class: 'block max-w-md truncate', text: description }
      });
    }
  );

  it.each([true, false])('gates edit/delete on canEdit=%s', (canEdit) => {
    const { props } = actions(canEdit);
    expect(props.edit).toBe('/eggs/egg-1');
    expect(props.edit_disabled).toBe(!canEdit);
    expect(props.delete_disabled).toBe(!canEdit);
  });

  it('calls deleteEgg with the row', () => {
    const { deleteEgg, props } = actions(true);
    (props.remove as () => void)();
    expect(deleteEgg).toHaveBeenCalledWith(egg);
  });
});
