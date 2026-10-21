import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { toast as sonner } from 'svelte-sonner';
import { goto, invalidate } from '$app/navigation';
import { toast } from '@profidev/pleiades/components/util/general';
import { createNode, deleteNode } from '$lib/client';
import ListPage from '$routes/nodes/+page.svelte';
import CreatePage from '$routes/nodes/create/+page.svelte';
import { adminUser, nodeList } from '$mocks/e2e/data';
import { Permission } from '$lib/permissions.svelte';

vi.mock('@profidev/pleiades/components/util/general', async (orig) => ({
  ...(await orig<object>()),
  toast: { error: vi.fn(), success: vi.fn() }
}));

vi.mock('$app/forms', async (orig) => ({
  ...(await orig<object>()),
  applyAction: vi.fn(async () => {})
}));

vi.mock('$lib/client', async (orig) => ({
  ...(await orig<object>()),
  createNode: vi.fn(),
  deleteNode: vi.fn()
}));

afterEach(cleanup);

// BaseForm / FormDialog report submit errors through svelte-sonner directly.
beforeEach(() => {
  vi.spyOn(sonner, 'error').mockImplementation(() => 0);
  sessionStorage.clear();
});

const tick = async (ms = 0) => new Promise((r) => setTimeout(r, ms));

const result = (status: number, body: unknown) =>
  ({
    data: status === 200 ? body : undefined,
    error: status === 200 ? undefined : body,
    request: new Request('http://x'),
    response: new Response(null, { status })
  }) as never;

const readonlyUser = {
  ...adminUser,
  permissions: adminUser.permissions.filter((p) => p !== Permission.NODE_EDIT)
};

const nodes = nodeList.default;

const listData = (
  list: unknown[] | undefined,
  user = adminUser,
  error: string | null = null
) => ({
  error,
  nodes: Promise.resolve(list),
  user: Promise.resolve(user)
});

const renderList = (data = listData(nodes)) =>
  render(ListPage, { props: { data } as never });

describe('nodes list page', () => {
  it('shows the header, rows and count', async () => {
    renderList();
    expect(screen.getByRole('heading', { name: 'Nodes' })).toBeInTheDocument();
    expect(await screen.findByText('2 nodes')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Node One' })).toHaveAttribute(
      'href',
      '/nodes/node-1'
    );
    expect(screen.getByText('node1.example.com:443')).toBeInTheDocument();
    expect(screen.getByText('10.0.0.2:8000')).toBeInTheDocument();
  });

  it('uses the singular for one node', async () => {
    renderList(listData(nodes.slice(0, 1)));
    expect(await screen.findByText('1 node')).toBeInTheDocument();
  });

  it('shows no count or empty state while loading', () => {
    render(ListPage, {
      props: {
        data: {
          error: null,
          nodes: new Promise(() => {}),
          user: Promise.resolve(adminUser)
        }
      } as never
    });
    expect(screen.queryByText(/\d+ nodes?$/)).not.toBeInTheDocument();
    expect(screen.queryByText('No nodes yet')).not.toBeInTheDocument();
  });

  it.each([[[]], [undefined]])(
    'shows the empty state for %j with one create button and no table',
    async (list) => {
      renderList(listData(list));
      expect(await screen.findByText('No nodes yet')).toBeInTheDocument();
      expect(screen.getAllByRole('link', { name: 'Create' })).toHaveLength(1);
      expect(screen.queryByRole('table')).not.toBeInTheDocument();
    }
  );

  it('searches by name and address', async () => {
    renderList();
    await screen.findByText('2 nodes');
    const search = screen.getByPlaceholderText('Search...');
    await fireEvent.input(search, { target: { value: 'Two' } });
    expect(screen.queryByRole('link', { name: 'Node One' })).toBeNull();
    await fireEvent.input(search, { target: { value: 'node1.example' } });
    expect(screen.getByRole('link', { name: 'Node One' })).toBeInTheDocument();
    expect(screen.queryByRole('link', { name: 'Node Two' })).toBeNull();
  });

  it('enables create for node:edit', async () => {
    renderList();
    await screen.findByText('2 nodes');
    await vi.waitFor(() =>
      expect(screen.getByRole('link', { name: 'Create' })).not.toHaveAttribute(
        'aria-disabled',
        'true'
      )
    );
  });

  it('disables create, edit and delete without node:edit', async () => {
    renderList(listData(nodes, readonlyUser));
    await screen.findByText('2 nodes');
    await vi.waitFor(() =>
      expect(screen.getByRole('link', { name: 'Create' })).toHaveAttribute(
        'aria-disabled',
        'true'
      )
    );
    const row = screen.getByRole('link', { name: 'Node One' }).closest('tr')!;
    const buttons = row.querySelectorAll('button');
    expect(buttons[buttons.length - 1]).toBeDisabled();
  });

  it.each([
    ['not_found', 'Node not found'],
    ['other', 'Failed to load node']
  ])('toasts ?error=%s and strips it from the URL', async (error, message) => {
    window.history.replaceState({}, '', `/nodes?error=${error}&x=1`);
    renderList(listData(nodes, adminUser, error));
    await vi.waitFor(() => expect(toast.error).toHaveBeenCalledWith(message));
    expect(window.location.search).toBe('?x=1');
  });

  it('strips an unknown error without a toast', async () => {
    window.history.replaceState({}, '', '/nodes?error=weird');
    renderList(listData(nodes, adminUser, 'weird'));
    await vi.waitFor(() => expect(window.location.search).toBe(''));
    expect(toast.error).not.toHaveBeenCalled();
  });

  describe('delete', () => {
    const openDelete = async () => {
      renderList();
      await screen.findByText('2 nodes');
      const row = screen.getByRole('link', { name: 'Node Two' }).closest('tr')!;
      const buttons = row.querySelectorAll('button');
      await vi.waitFor(() =>
        expect(buttons[buttons.length - 1]).not.toBeDisabled()
      );
      await fireEvent.click(buttons[buttons.length - 1]);
      expect(
        await screen.findByText('Delete node Node Two?')
      ).toBeInTheDocument();
    };

    it('deletes, toasts and invalidates on success', async () => {
      vi.mocked(deleteNode).mockResolvedValue(result(200, undefined));
      await openDelete();
      await fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
      await vi.waitFor(() =>
        expect(toast.success).toHaveBeenCalledWith(
          'Node Node Two deleted successfully'
        )
      );
      expect(deleteNode).toHaveBeenCalledWith({ body: { uuid: 'node-2' } });
      const pred = vi.mocked(invalidate).mock.calls[0][0] as (
        u: URL
      ) => boolean;
      expect(pred(new URL('http://x/api/nodes'))).toBe(true);
      expect(pred(new URL('http://x/api/eggs'))).toBe(false);
    });

    it.each([
      [409, 'Node still hosts servers. Delete or transfer them first.'],
      [500, 'Failed to delete node']
    ])('shows an error on %i', async (status, message) => {
      vi.mocked(deleteNode).mockResolvedValue(result(status, ''));
      await openDelete();
      await fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
      await vi.waitFor(() =>
        expect(sonner.error).toHaveBeenCalledWith(message)
      );
      expect(screen.getByText('Delete node Node Two?')).toBeInTheDocument();
      expect(toast.success).not.toHaveBeenCalled();
      expect(invalidate).not.toHaveBeenCalled();
    });
  });
});

describe('node create page', () => {
  const next = async () => {
    await fireEvent.click(screen.getByRole('button', { name: /^Next/ }));
    await tick(50);
  };

  const fillWizard = async () => {
    render(CreatePage);
    await fireEvent.input(screen.getByPlaceholderText('Enter name'), {
      target: { value: 'Fresh' }
    });
    await fireEvent.input(
      screen.getByPlaceholderText('Enter address (e.g., IP or domain)'),
      { target: { value: 'fresh.example.com:8443' } }
    );
    await next();
    await screen.findByRole('switch', { name: 'Unlimited CPU' });
  };

  const submit = async () => {
    await fireEvent.click(screen.getByRole('button', { name: 'Create' }));
    await tick(50);
  };

  it('shows the header with a back link and the General stage', () => {
    render(CreatePage);
    expect(
      screen.getByRole('heading', { name: 'Create node' })
    ).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Back to nodes' })).toHaveAttribute(
      'href',
      '/nodes'
    );
    expect(screen.getByText('General')).toBeInTheDocument();
  });

  it('warns about plain HTTP when secure is off', async () => {
    render(CreatePage);
    expect(
      screen.queryByText('Only use HTTP on private networks!')
    ).not.toBeInTheDocument();
    await fireEvent.click(
      screen.getByRole('switch', { name: 'Use Secure Connection (HTTPS)' })
    );
    expect(
      await screen.findByText('Only use HTTP on private networks!')
    ).toBeInTheDocument();
  });

  it('shows the limit inputs only when a limit is set', async () => {
    await fillWizard();
    expect(screen.getByText('Resources')).toBeInTheDocument();
    expect(
      screen.queryByPlaceholderText('Enter CPU limit')
    ).not.toBeInTheDocument();
    await fireEvent.click(
      screen.getByRole('switch', { name: 'Unlimited CPU' })
    );
    expect(
      await screen.findByPlaceholderText('Enter CPU limit')
    ).toBeInTheDocument();
  });

  it('creates the node, stores the token and opens its setup tab', async () => {
    vi.mocked(createNode).mockResolvedValue(
      result(200, { token: 'tok', uuid: 'node-new' })
    );
    await fillWizard();
    await submit();
    await vi.waitFor(() =>
      expect(goto).toHaveBeenCalledWith('/nodes/node-new?tab=setup')
    );
    expect(createNode).toHaveBeenCalledWith({
      body: {
        address: 'fresh.example.com:8443',
        name: 'Fresh',
        secure: true
      }
    });
    expect(toast.success).toHaveBeenCalledWith('Node created successfully.');
    expect(sessionStorage.getItem('newNodeToken')).toBe('tok');
  });

  it.each([
    [409, 'A node with this name already exists.'],
    [400, 'Invalid node address.'],
    [500, 'Error creating node.']
  ])('shows the %i error', async (status, message) => {
    vi.mocked(createNode).mockResolvedValue(result(status, ''));
    await fillWizard();
    await submit();
    await vi.waitFor(() => expect(sonner.error).toHaveBeenCalledWith(message));
    expect(goto).not.toHaveBeenCalled();
    expect(sessionStorage.getItem('newNodeToken')).toBeNull();
  });
});
