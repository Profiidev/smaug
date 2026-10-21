import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within
} from '@testing-library/svelte';
import { toast as sonner } from 'svelte-sonner';
import { goto } from '$app/navigation';
import { toast } from '@profidev/pleiades/components/util/general';
import {
  type NodeInfo,
  deleteNode,
  nodeTokenRegenerate,
  updateNode
} from '$lib/client';
import EditPage from '$routes/nodes/[uuid]/[tab=nodetab]/+page.svelte';
import { page } from '$test_helpers/page-state.svelte';
import { adminUser } from '$mocks/e2e/data';
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
  deleteNode: vi.fn(),
  nodeTokenRegenerate: vi.fn(),
  updateNode: vi.fn()
}));

vi.mock('$app/state', async () => import('$test_helpers/page-state.svelte'));

afterEach(cleanup);

beforeEach(() => {
  page.params = { tab: 'setup', uuid: 'node-1' };
  // Navigating to a tab URL updates the route param like SvelteKit does.
  vi.mocked(goto).mockImplementation(async (url) => {
    const [uuid, tab] = String(url).split('/').slice(2);
    if (tab) {
      page.params = { tab, uuid };
    }
  });
  vi.spyOn(sonner, 'error').mockImplementation(() => 0);
  sessionStorage.clear();
});

const tick = async (ms = 0) => new Promise((r) => setTimeout(r, ms));

const result = (status: number, body?: unknown) =>
  ({
    data: status === 200 ? (body ?? {}) : undefined,
    error: status === 200 ? undefined : body,
    request: new Request('http://x'),
    response: new Response(null, { status })
  }) as never;

const node: NodeInfo = {
  address: 'node1.example.com',
  connected: true,
  id: 'node-1',
  name: 'Node One',
  port: 443,
  secure: true
};

const readonlyUser = {
  ...adminUser,
  permissions: adminUser.permissions.filter((p) => p !== Permission.NODE_EDIT)
};

const pageData = (info: NodeInfo = node, { user = adminUser } = {}) => ({
  nodeRes: Promise.resolve({
    data: info,
    response: new Response(null, { status: 200 })
  }),
  user: Promise.resolve(user),
  uuid: 'node-1'
});

const mount = async (data = pageData()) => {
  render(EditPage, { props: { data } as never });
  await screen.findByRole('heading', { name: 'Node: Node One' });
  await tick();
};

const openTab = async (name: string) => {
  const tab = screen.getByRole('tab', { name: new RegExp(`^${name}`) });
  await fireEvent.mouseDown(tab);
  await fireEvent.click(tab);
  await tick();
};

const save = async () => {
  await fireEvent.click(screen.getByRole('button', { name: 'Save Changes' }));
  await tick(50);
};

const nameInput = () => screen.getByPlaceholderText('Enter name');
const saveBar = () =>
  screen.getByRole('button', { name: 'Save Changes' }).parentElement!
    .parentElement!;

describe('node edit page loading', () => {
  it('renders tabs and a disabled form until the node loads', () => {
    render(EditPage, {
      props: {
        data: {
          nodeRes: new Promise(() => {}),
          user: Promise.resolve(adminUser),
          uuid: 'node-1'
        }
      } as never
    });
    expect(screen.getByRole('heading', { name: 'Node:' })).toBeInTheDocument();
    expect(
      screen.getAllByRole('tab').map((t) => t.textContent?.trim())
    ).toEqual(['Setup', 'General', 'Resources']);
    expect(screen.getByRole('button', { name: /Save Changes/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Delete' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Regenerate' })).toBeDisabled();
    expect(nameInput()).toBeDisabled();
    expect(nameInput()).toHaveValue('');
    expect(screen.getByRole('link', { name: 'Back to nodes' })).toHaveAttribute(
      'href',
      '/nodes'
    );
  });

  it.each([
    [404, '/nodes?error=not_found'],
    [500, '/nodes?error=other'],
    [403, '/nodes?error=other']
  ])('redirects on %i', async (status, target) => {
    render(EditPage, {
      props: {
        data: {
          ...pageData(),
          nodeRes: Promise.resolve({
            data: undefined,
            response: new Response(null, { status })
          })
        }
      } as never
    });
    await vi.waitFor(() => expect(goto).toHaveBeenCalledWith(target));
  });
});

describe('node edit page tabs', () => {
  it('opens Setup by default and hides the save bar there', async () => {
    await mount();
    expect(screen.getByRole('tab', { name: 'Setup' })).toHaveAttribute(
      'data-state',
      'active'
    );
    expect(saveBar()).toHaveClass('hidden');
    await openTab('General');
    expect(saveBar()).not.toHaveClass('hidden');
  });

  it.each([
    ['general', 'General'],
    ['resources', 'Resources'],
    ['setup', 'Setup']
  ])('opens the /%s param as %s', async (tab, name) => {
    page.params = { tab, uuid: 'node-1' };
    await mount();
    expect(screen.getByRole('tab', { name })).toHaveAttribute(
      'data-state',
      'active'
    );
  });

  it('defaults to Setup without a tab param', async () => {
    page.params = { uuid: 'node-1' };
    await mount();
    expect(screen.getByRole('tab', { name: 'Setup' })).toHaveAttribute(
      'data-state',
      'active'
    );
  });

  it('navigates to the tab URL keeping focus and scroll', async () => {
    await mount();
    await openTab('Resources');
    expect(goto).toHaveBeenCalledExactlyOnceWith('/nodes/node-1/resources', {
      keepFocus: true,
      noScroll: true
    });
    expect(screen.getByRole('tab', { name: 'Resources' })).toHaveAttribute(
      'data-state',
      'active'
    );
  });

  it('does not navigate when clicking the open tab', async () => {
    await mount();
    await openTab('Setup');
    expect(goto).not.toHaveBeenCalled();
  });

  it('keeps unsaved edits when switching tabs', async () => {
    await mount();
    await openTab('General');
    await fireEvent.input(nameInput(), { target: { value: 'Draft' } });
    expect(await screen.findByText('Unsaved changes')).toBeInTheDocument();
    await openTab('Resources');
    await openTab('Setup');
    await openTab('General');
    expect(nameInput()).toHaveValue('Draft');
    expect(screen.getByText('Unsaved changes')).toBeInTheDocument();
  });

  it('fills the form from the node', async () => {
    await mount(pageData({ ...node, cpu_limit: 1500, memory_limit_mb: 1 }));
    expect(nameInput()).toHaveValue('Node One');
    expect(nameInput()).toBeEnabled();
    expect(
      screen.getByPlaceholderText('Enter address (e.g., IP or domain)')
    ).toHaveValue('node1.example.com:443');
    expect(screen.getByPlaceholderText('Enter CPU limit')).toHaveValue(1500);
    expect(screen.getByPlaceholderText('Enter amount of memory')).toHaveValue(
      1
    );
    expect(
      screen.queryByPlaceholderText('Enter amount of storage')
    ).not.toBeInTheDocument();
  });

  it('shows the connected status', async () => {
    await mount(pageData({ ...node, connected: false }));
    expect(
      document.querySelector('h3 ~ button.text-red-500')
    ).toBeInTheDocument();
  });
});

describe('node edit page readonly', () => {
  it('disables fields, save, delete and regenerate without node:edit', async () => {
    await mount(pageData(node, { user: readonlyUser }));
    expect(nameInput()).toBeDisabled();
    expect(
      screen.getByRole('switch', { hidden: true, name: 'Unlimited CPU' })
    ).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Save Changes' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Delete' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Regenerate' })).toBeDisabled();
  });
});

describe('node edit page save', () => {
  it('saves, renames the header and keeps the values', async () => {
    vi.mocked(updateNode).mockResolvedValue(result(200));
    await mount();
    await openTab('General');
    await fireEvent.input(nameInput(), { target: { value: 'Renamed' } });
    expect(await screen.findByText('Unsaved changes')).toBeInTheDocument();
    await save();
    await vi.waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith(
        'Node Renamed updated successfully'
      )
    );
    expect(updateNode).toHaveBeenCalledWith({
      body: {
        address: 'node1.example.com:443',
        name: 'Renamed',
        secure: true
      },
      path: { uuid: 'node-1' }
    });
    expect(
      screen.getByRole('heading', { name: 'Node: Renamed' })
    ).toBeInTheDocument();
    await vi.waitFor(() =>
      expect(screen.queryByText('Unsaved changes')).not.toBeInTheDocument()
    );
    expect(nameInput()).toHaveValue('Renamed');
  });

  it.each([
    [409, 'This node name is already in use'],
    [400, 'Invalid node address']
  ])(
    'shows the %i field error on General with a toast',
    async (status, message) => {
      vi.mocked(updateNode).mockResolvedValue(result(status, ''));
      page.params = { tab: 'resources', uuid: 'node-1' };
      await mount();
      await save();
      await vi.waitFor(() =>
        expect(toast.error).toHaveBeenCalledWith("Couldn't save node", {
          description: message
        })
      );
      expect(await screen.findByText(message)).toBeInTheDocument();
      const tab = screen.getByRole('tab', { name: /^General/ });
      expect(tab).toHaveAttribute('data-state', 'active');
      expect(within(tab).getByLabelText('Has errors')).toBeInTheDocument();
      expect(
        screen.getByRole('button', { name: 'Save Changes' })
      ).toBeEnabled();
      expect(toast.success).not.toHaveBeenCalled();
    }
  );

  it('shows a retry button on other errors', async () => {
    vi.mocked(updateNode).mockResolvedValue(result(500, ''));
    page.params = { tab: 'general', uuid: 'node-1' };
    await mount();
    await save();
    await vi.waitFor(() =>
      expect(sonner.error).toHaveBeenCalledWith('Failed to update node')
    );
    expect(
      await screen.findByRole('button', { name: 'Retry' })
    ).toBeInTheDocument();
    expect(toast.success).not.toHaveBeenCalled();
  });
});

describe('node edit page setup tab', () => {
  it('shows the token once from sessionStorage', async () => {
    sessionStorage.setItem('newNodeToken', 'fresh-token');
    await mount();
    expect(screen.getByText('fresh-token')).toBeInTheDocument();
    expect(screen.getByText('(Can not be viewed again!)')).toBeInTheDocument();
    expect(sessionStorage.getItem('newNodeToken')).toBeNull();
  });

  it('hides the token without one stored', async () => {
    await mount();
    expect(screen.getByLabelText('Node Auth Token')).toHaveValue(
      'Can not be viewed.'
    );
    expect(
      screen.queryByText('(Can not be viewed again!)')
    ).not.toBeInTheDocument();
  });

  const regenerate = async () => {
    await mount();
    await fireEvent.click(screen.getByRole('button', { name: 'Regenerate' }));
    const dialog = await screen.findByRole('dialog');
    expect(
      within(dialog).getByText('Regenerate token for Node One?')
    ).toBeInTheDocument();
    await fireEvent.click(
      within(dialog).getByRole('button', { name: 'Regenerate' })
    );
    await tick(50);
  };

  it('regenerates the token', async () => {
    vi.mocked(nodeTokenRegenerate).mockResolvedValue(
      result(200, { token: 'new-token' })
    );
    await regenerate();
    await vi.waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith(
        'Token for node Node One regenerated successfully'
      )
    );
    expect(nodeTokenRegenerate).toHaveBeenCalledWith({
      path: { uuid: 'node-1' }
    });
    expect(await screen.findByText('new-token')).toBeInTheDocument();
  });

  it('shows an error when regenerate fails', async () => {
    vi.mocked(nodeTokenRegenerate).mockResolvedValue(result(500, ''));
    await regenerate();
    await vi.waitFor(() =>
      expect(sonner.error).toHaveBeenCalledWith('Failed to regenerate token')
    );
    expect(toast.success).not.toHaveBeenCalled();
  });
});

describe('node edit page delete', () => {
  const openDelete = async () => {
    await mount();
    await fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    expect(
      await screen.findByText('Delete node Node One?')
    ).toBeInTheDocument();
    const dialog = screen.getByRole('dialog');
    await fireEvent.click(
      within(dialog).getByRole('button', { name: 'Delete' })
    );
    await tick(50);
  };

  it('deletes and returns to the list', async () => {
    vi.mocked(deleteNode).mockResolvedValue(result(200));
    await openDelete();
    await vi.waitFor(() => expect(goto).toHaveBeenCalledWith('/nodes'));
    expect(deleteNode).toHaveBeenCalledWith({ body: { uuid: 'node-1' } });
    expect(toast.success).toHaveBeenCalledWith(
      'Node Node One deleted successfully'
    );
  });

  it('shows the in-use error on 409', async () => {
    vi.mocked(deleteNode).mockResolvedValue(result(409, ''));
    await openDelete();
    await vi.waitFor(() =>
      expect(sonner.error).toHaveBeenCalledWith(
        'Node still hosts servers. Delete or transfer them first.'
      )
    );
    expect(goto).not.toHaveBeenCalled();
  });
});
