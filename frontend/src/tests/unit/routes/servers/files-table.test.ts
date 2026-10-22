import { describe, expect, it, vi } from 'vitest';
import type { FileEntry } from '$lib/client';
import {
  type FileColumnData,
  columns,
  sizeText
} from '$routes/servers/[uuid]/files-table.svelte';

vi.mock('@tanstack/svelte-table', async (orig) => ({
  ...(await orig<object>()),
  renderComponent: (component: unknown, props: unknown) => ({
    component,
    props
  })
}));

interface Cell {
  props: Record<string, unknown>;
}

const file: FileEntry = {
  kind: 'file',
  mode: 0o644,
  modified: new Date('2026-01-01T00:00:00Z'),
  name: 'config.yml',
  size: 2048
};
const dir: FileEntry = { ...file, kind: 'dir', name: 'plugins', size: 4096 };

const data = (over: Partial<FileColumnData> = {}): FileColumnData => ({
  allSelected: false,
  canWrite: true,
  dirHref: (e) => (e.kind === 'dir' ? `?path=/${e.name}` : undefined),
  downloadHref: () => '/dl',
  editHref: () => '/edit',
  oncopy: vi.fn(),
  ondelete: vi.fn(),
  onrename: vi.fn(),
  selected: [],
  toggle: vi.fn(),
  toggleAll: vi.fn(),
  ...over
});

const keys = (d: FileColumnData) =>
  columns(d).map((c) => ('accessorKey' in c ? c.accessorKey : undefined));

const cell = (d: FileColumnData, key: string, entry: FileEntry) => {
  const col = columns(d).find(
    (c) => 'accessorKey' in c && c.accessorKey === key
  )!;
  return (col.cell as (ctx: unknown) => Cell)({ row: { original: entry } });
};

describe('file columns', () => {
  it('has a select column only with write access', () => {
    expect(keys(data())).toEqual([
      'select',
      'name',
      'size',
      'modified',
      'actions'
    ]);
    expect(keys(data({ canWrite: false }))).toEqual([
      'name',
      'size',
      'modified',
      'actions'
    ]);
  });

  it('checks selected rows and forwards toggles', () => {
    const d = data({ selected: ['config.yml'] });
    const c = cell(d, 'select', file);
    expect(c.props.checked).toBe(true);
    (c.props.onCheckedChange as (on: boolean) => void)(false);
    expect(d.toggle).toHaveBeenCalledWith('config.yml', false);
    expect(cell(d, 'select', dir).props.checked).toBe(false);
  });

  it('shows an indeterminate header for a partial selection', () => {
    const header = (d: FileColumnData) =>
      (columns(d)[0].header as () => Cell)().props;
    expect(header(data({ selected: ['a'] }))).toMatchObject({
      checked: false,
      indeterminate: true
    });
    expect(header(data({ allSelected: true, selected: ['a'] }))).toMatchObject({
      checked: true,
      indeterminate: false
    });
  });

  it('links folders and shows a dash for their size', () => {
    expect(cell(data(), 'name', dir).props.href).toBe('?path=/plugins');
    expect(cell(data(), 'name', file).props.href).toBeUndefined();
    expect(sizeText(dir)).toBe('—');
    expect(sizeText(file)).toBe('2.0 KiB');
  });

  it('puts the absolute time in the title', () => {
    const c = cell(data(), 'modified', file);
    expect(c.props.title).toBe(file.modified.toLocaleString());
  });

  it('passes access and links to the actions', () => {
    const c = cell(data({ canWrite: false }), 'actions', file);
    expect(c.props).toMatchObject({
      canWrite: false,
      downloadHref: '/dl',
      editHref: '/edit',
      entry: file
    });
  });
});
