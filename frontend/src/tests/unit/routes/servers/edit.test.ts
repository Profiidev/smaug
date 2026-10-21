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
import { type ServerInfo, deleteServer, updateServer } from '$lib/client';
import EditPage from '$routes/servers/[uuid]/[tab=servertab]/+page.svelte';
import { fromServerInfo, toUpdateBody } from '$routes/servers/schema.svelte';
import { page } from '$test_helpers/page-state.svelte';
import { options, serverInfo } from './fixtures';

vi.mock('$app/state', async () => import('$test_helpers/page-state.svelte'));

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
  deleteServer: vi.fn(),
  updateServer: vi.fn()
}));

afterEach(cleanup);

beforeEach(() => {
  vi.spyOn(sonner, 'error').mockImplementation(() => 0);
  page.params = { tab: 'status', uuid: 'srv-1' };
  // Navigating to a tab URL updates the route param like SvelteKit does.
  vi.mocked(goto).mockImplementation(async (url) => {
    const [, root, uuid, tab] = String(url).split('/');
    if (root === 'servers' && tab) {
      page.params = { tab, uuid };
    }
  });
});

const tick = async (ms = 0) => new Promise((r) => setTimeout(r, ms));

const result = (status: number, body?: unknown) =>
  ({
    data: status === 200 ? (body ?? {}) : undefined,
    error: status === 200 ? undefined : body,
    request: new Request('http://x'),
    response: new Response(null, { status })
  }) as never;

const info = (over: Partial<ServerInfo> = {}): ServerInfo => ({
  ...structuredClone(serverInfo),
  ...over
});

const asAccess = (access: ServerInfo['access']) =>
  info({ access, grants: access === 'admin' ? serverInfo.grants : null });

const pageData = (server: ServerInfo | undefined, status = 200) => ({
  options: Promise.resolve(server?.access === 'admin' ? options : undefined),
  serverRes: Promise.resolve({
    data: server,
    response: new Response(null, { status })
  }),
  uuid: 'srv-1'
});

const mount = async (server: ServerInfo = info()) => {
  render(EditPage, { props: { data: pageData(server) } as never });
  // Tabs render immediately; wait for the loaded server instead.
  await screen.findByRole('heading', { name: `Server: ${server.name}` });
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

const comboboxes = () =>
  screen
    .getAllByRole('combobox', { hidden: true })
    .filter((b) => b.tagName === 'BUTTON' && b.closest('[role=tabpanel]'));

describe('server edit page loading', () => {
  it('renders tabs and a disabled, empty form until the server loads', () => {
    render(EditPage, {
      props: {
        data: {
          options: new Promise(() => {}),
          serverRes: new Promise(() => {}),
          uuid: 'srv-1'
        }
      } as never
    });
    expect(
      screen.getByRole('heading', { name: 'Server:' })
    ).toBeInTheDocument();
    expect(document.querySelector('[aria-busy=true]')).toBeNull();
    expect(
      screen.getAllByRole('tab').map((t) => t.textContent?.trim())
    ).toEqual(['Status', 'General', 'Startup', 'Variables', 'Resources']);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Save Changes/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Delete' })).toBeDisabled();
    expect(
      screen.getByRole('link', { name: 'Back to servers' })
    ).toHaveAttribute('href', '/servers');
  });

  it.each([
    [404, '/servers?error=not_found'],
    [500, '/servers?error=other'],
    [403, '/servers?error=other']
  ])('redirects on %i', async (status, target) => {
    render(EditPage, {
      props: { data: pageData(undefined, status) } as never
    });
    await vi.waitFor(() => expect(goto).toHaveBeenCalledWith(target));
  });
});

describe('server edit page access', () => {
  it('admin: header, every tab and enabled fields', async () => {
    await mount();
    expect(
      screen.getByRole('heading', { name: 'Server: survival' })
    ).toBeInTheDocument();
    expect(
      screen.getAllByRole('tab').map((t) => t.textContent?.trim())
    ).toEqual([
      'Status',
      'General',
      'Startup',
      'Variables',
      'Resources',
      'Permissions'
    ]);
    expect(screen.getByRole('tab', { name: 'Status' })).toHaveAttribute(
      'data-state',
      'active'
    );
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Delete' })).toBeEnabled();
    expect(screen.getByLabelText('Name')).toBeEnabled();
    expect(screen.getByLabelText('Image')).toBeEnabled();
    expect(
      screen.getByRole('switch', { hidden: true, name: 'Unlimited CPU' })
    ).toBeEnabled();
    for (const b of comboboxes()) {
      if (b.textContent?.trim() !== 'eu-1') {
        expect(b).toBeEnabled();
      }
    }
    // The node only changes through Transfer.
    expect(
      comboboxes().find((b) => b.textContent?.trim() === 'eu-1')
    ).toBeDisabled();
    for (const name of ['Add user', 'Add group']) {
      expect(screen.getByRole('button', { hidden: true, name })).toBeEnabled();
    }
    expect(screen.getByRole('button', { name: 'Transfer…' })).toBeEnabled();
  });

  it('fills the selects from the loaded options', async () => {
    await mount();
    await openTab('Startup');
    expect(screen.getByLabelText('Image name').textContent?.trim()).toBe(
      'Java 21'
    );
    expect(screen.getByLabelText('Startup command').textContent?.trim()).toBe(
      'Custom'
    );
  });

  it('hides the save bar on the Status tab only', async () => {
    await mount();
    const bar = () =>
      screen.getByRole('button', { name: 'Save Changes' }).parentElement!
        .parentElement!;
    expect(bar()).toHaveClass('hidden');
    await openTab('General');
    expect(bar()).not.toHaveClass('hidden');
  });

  it('write: limited alert, no Permissions tab, admin fields locked', async () => {
    await mount(asAccess('write'));
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Some settings need an administrator'
    );
    expect(
      screen.queryByRole('tab', { name: 'Permissions' })
    ).not.toBeInTheDocument();
    expect(screen.getByLabelText('Name')).toBeEnabled();
    expect(screen.getByLabelText('Command')).toBeEnabled();
    expect(screen.getByLabelText('Server Jar File')).toBeEnabled();
    expect(screen.getByLabelText('Max players')).toBeDisabled();
    expect(screen.getByLabelText('Image')).toBeDisabled();
    expect(
      screen.getByRole('switch', { hidden: true, name: 'Unlimited CPU' })
    ).toBeDisabled();
    expect(screen.getByLabelText('Backup limit')).toBeDisabled();
    for (const b of comboboxes()) {
      expect(b).toBeDisabled();
    }
    expect(comboboxes().map((b) => b.textContent?.trim())).toEqual([
      'Ada Admin',
      'eu-1',
      'Paper'
    ]);
    expect(screen.getByRole('button', { name: 'Save Changes' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Delete' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Reinstall' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Transfer…' })).toBeDisabled();
  });

  it('read: view-only alert and everything disabled', async () => {
    await mount(asAccess('read'));
    expect(screen.getByRole('alert')).toHaveTextContent(
      'You have view-only access'
    );
    expect(
      screen.queryByRole('tab', { name: 'Permissions' })
    ).not.toBeInTheDocument();
    for (const label of ['Name', 'Command', 'Server Jar File', 'Image']) {
      expect(screen.getByLabelText(label)).toBeDisabled();
    }
    expect(screen.getByRole('button', { name: 'Save Changes' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Delete' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Reinstall' })).toBeDisabled();
  });

  it('disables Delete while installing', async () => {
    await mount(info({ install_state: 'installing', status: 'installing' }));
    expect(screen.getByRole('button', { name: 'Delete' })).toBeDisabled();
    expect(
      screen.getByRole('button', { name: 'Delete server' })
    ).toBeDisabled();
  });
});

describe('server edit page save', () => {
  it('saves the whole form with grants for admins', async () => {
    vi.mocked(updateServer).mockResolvedValue(result(200));
    await mount();
    await openTab('General');
    await fireEvent.input(screen.getByLabelText('Name'), {
      target: { value: 'renamed' }
    });
    expect(await screen.findByText('Unsaved changes')).toBeInTheDocument();
    await save();
    await vi.waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith(
        'Server renamed updated successfully'
      )
    );
    const expected = toUpdateBody(
      { ...fromServerInfo(info()), name: 'renamed' },
      true
    );
    expect(updateServer).toHaveBeenCalledWith({
      body: expected,
      path: { uuid: 'srv-1' }
    });
    expect(expected.grants).toBeDefined();
    expect(
      screen.getByRole('heading', { name: 'Server: renamed' })
    ).toBeInTheDocument();
    await vi.waitFor(() =>
      expect(screen.queryByText('Unsaved changes')).not.toBeInTheDocument()
    );
    expect(screen.getByLabelText('Name')).toHaveValue('renamed');
  });

  it('sends no grants for write access', async () => {
    vi.mocked(updateServer).mockResolvedValue(result(200));
    await mount(asAccess('write'));
    await openTab('General');
    await save();
    await vi.waitFor(() => expect(updateServer).toHaveBeenCalled());
    const [[{ body }]] = vi.mocked(updateServer).mock.calls;
    expect(body).not.toHaveProperty('grants');
    expect(body.environment).toEqual({ SERVER_JARFILE: 'paper.jar' });
  });

  it('shows 400 field errors on their tab and opens the first', async () => {
    vi.mocked(updateServer).mockResolvedValue(
      result(400, {
        errors: [
          { field: 'limits.cpu', message: 'CPU too high.' },
          { field: 'variables.SERVER_JARFILE', message: 'Bad jar.' }
        ]
      })
    );
    await mount();
    await openTab('General');
    await save();
    await vi.waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith("Couldn't save server", {
        description: 'Fix the highlighted fields.'
      })
    );
    expect(await screen.findByText('Bad jar.')).toBeInTheDocument();
    expect(screen.getByText('CPU too high.')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: /^Variables/ })).toHaveAttribute(
      'data-state',
      'active'
    );
    expect(goto).toHaveBeenLastCalledWith('/servers/srv-1/variables', {
      keepFocus: true,
      noScroll: true,
      replaceState: false
    });
    for (const name of ['Variables', 'Resources']) {
      const tab = screen.getByRole('tab', { name: new RegExp(`^${name}`) });
      expect(within(tab).getByLabelText('Has errors')).toBeInTheDocument();
    }
    expect(
      within(screen.getByRole('tab', { name: /^General/ })).queryByLabelText(
        'Has errors'
      )
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Save Changes' })
    ).toBeInTheDocument();
    expect(toast.success).not.toHaveBeenCalled();
  });

  it('shows a 409 allocation conflict on General', async () => {
    vi.mocked(updateServer).mockResolvedValue(
      result(409, {
        errors: [{ field: 'allocations.1', message: 'Allocation in use.' }]
      })
    );
    await mount();
    await openTab('Resources');
    await save();
    await vi.waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith("Couldn't save server", {
        description: 'An allocation is already in use.'
      })
    );
    expect(await screen.findByText('Allocation in use.')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: /^General/ })).toHaveAttribute(
      'data-state',
      'active'
    );
    expect(goto).toHaveBeenLastCalledWith('/servers/srv-1/general', {
      keepFocus: true,
      noScroll: true,
      replaceState: false
    });
  });

  it('shows grant field errors on their row in Permissions', async () => {
    vi.mocked(updateServer).mockResolvedValue(
      result(400, {
        errors: [
          { field: 'grants.users.0', message: 'User is gone.' },
          { field: 'grants.groups.0', message: 'Group is gone.' }
        ]
      })
    );
    await mount();
    await openTab('General');
    await save();
    expect(await screen.findByText('User is gone.')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: /^Permissions/ })).toHaveAttribute(
      'data-state',
      'active'
    );
    expect(
      within(screen.getByRole('group', { name: 'User 1' })).getByText(
        'User is gone.'
      )
    ).toBeInTheDocument();
    expect(
      within(screen.getByRole('group', { name: 'Group 1' })).getByText(
        'Group is gone.'
      )
    ).toBeInTheDocument();
  });

  it('describes field errors it cannot place', async () => {
    vi.mocked(updateServer).mockResolvedValue(
      result(400, { errors: [{ field: 'mystery', message: 'Odd.' }] })
    );
    await mount();
    await openTab('General');
    await save();
    await vi.waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith("Couldn't save server", {
        description: 'Odd.'
      })
    );
  });

  it('explains a 403 as missing permission, without Retry', async () => {
    vi.mocked(updateServer).mockResolvedValue(result(403, 'nope'));
    await mount();
    await openTab('General');
    await save();
    await vi.waitFor(() =>
      expect(sonner.error).toHaveBeenCalledWith(
        "You don't have permission to change these fields."
      )
    );
    expect(
      screen.getByRole('button', { name: 'Save Changes' })
    ).toBeInTheDocument();
    expect(toast.success).not.toHaveBeenCalled();
  });

  // Owner is NULL once the owning user is deleted; the form must still save.
  it.each(['admin', 'write'] as const)(
    'saves a server without an owner as %s',
    async (access) => {
      vi.mocked(updateServer).mockResolvedValue(result(200));
      await mount({ ...asAccess(access), owner: null });
      await openTab('General');
      await save();
      await vi.waitFor(() => expect(updateServer).toHaveBeenCalled());
      const [[{ body }]] = vi.mocked(updateServer).mock.calls;
      expect(body.owner).toBeNull();
      expect(toast.success).toHaveBeenCalled();
    }
  );

  it('turns the save bar into Retry on 500', async () => {
    const status = 500;
    vi.mocked(updateServer).mockResolvedValue(result(status, 'nope'));
    await mount();
    await openTab('General');
    await save();
    await vi.waitFor(() =>
      expect(sonner.error).toHaveBeenCalledWith(
        "Couldn't save changes. Check your connection and retry."
      )
    );
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument();
    expect(toast.success).not.toHaveBeenCalled();
  });

  it('does not submit invalid fields', async () => {
    await mount();
    await openTab('General');
    await fireEvent.input(screen.getByLabelText('Name'), {
      target: { value: '' }
    });
    await save();
    await tick(150);
    expect(updateServer).not.toHaveBeenCalled();
  });

  it('reloads startup, image and variables after an egg change', async () => {
    vi.mocked(updateServer).mockResolvedValue(result(200));
    await mount();
    await openTab('General');
    const egg = comboboxes().find((b) => b.textContent?.trim() === 'Paper')!;
    await fireEvent.click(egg);
    await fireEvent.click(
      await screen.findByRole('option', { name: 'Vanilla' })
    );
    await tick();
    // The server's variables differ from the egg defaults, so it asks first.
    await fireEvent.click(
      await screen.findByRole('button', { name: 'Change egg' })
    );
    await tick(50);
    expect(screen.getByLabelText('Command')).toHaveValue('./run.sh');
    expect(screen.getByLabelText('Image')).toHaveValue('alpine:3');
    expect(screen.getByText('No variables')).toBeInTheDocument();
    await save();
    await vi.waitFor(() => expect(updateServer).toHaveBeenCalled());
    const [[{ body }]] = vi.mocked(updateServer).mock.calls;
    expect(body).toMatchObject({
      egg: 'egg-vanilla',
      environment: {},
      image: 'alpine:3',
      startup: './run.sh',
      startup_overridden: false
    });
  });
});

describe('server edit page reload', () => {
  const reload = async (
    rerender: (p: never) => unknown,
    server: ServerInfo
  ) => {
    await rerender({ data: pageData(server) } as never);
    await tick(50);
  };

  it('keeps unsaved edits but shows the new status', async () => {
    const { rerender } = render(EditPage, {
      props: { data: pageData(info()) } as never
    });
    await screen.findByRole('tab', { name: 'General' });
    await tick();
    await openTab('General');
    await fireEvent.input(screen.getByLabelText('Name'), {
      target: { value: 'renamed' }
    });
    expect(await screen.findByText('Unsaved changes')).toBeInTheDocument();
    await reload(rerender, info({ name: 'other', status: 'suspended' }));
    expect(screen.getByLabelText('Name')).toHaveValue('renamed');
    expect(screen.getByText('Unsaved changes')).toBeInTheDocument();
    expect(screen.getAllByText('Suspended').length).toBeGreaterThan(0);
  });

  it('takes the reloaded values while the form is clean', async () => {
    const { rerender } = render(EditPage, {
      props: { data: pageData(info()) } as never
    });
    await screen.findByRole('tab', { name: 'General' });
    await tick();
    await openTab('General');
    await reload(rerender, info({ name: 'other' }));
    expect(screen.getByLabelText('Name')).toHaveValue('other');
    expect(screen.queryByText('Unsaved changes')).not.toBeInTheDocument();
  });
});

describe('server edit page delete', () => {
  const confirmDelete = async (opener = 'Delete') => {
    await fireEvent.click(screen.getByRole('button', { name: opener }));
    const dialog = await screen.findByRole('dialog');
    expect(
      within(dialog).getByText('Delete server survival?')
    ).toBeInTheDocument();
    await fireEvent.click(
      within(dialog).getByRole('button', { name: 'Delete' })
    );
    await tick(50);
  };

  it.each(['Delete', 'Delete server'])(
    'deletes from "%s" and returns to the list',
    async (opener) => {
      vi.mocked(deleteServer).mockResolvedValue(result(200));
      await mount();
      await confirmDelete(opener);
      await vi.waitFor(() => expect(goto).toHaveBeenCalledWith('/servers'));
      expect(toast.success).toHaveBeenCalledWith(
        'Server survival deleted successfully'
      );
      expect(deleteServer).toHaveBeenCalledWith({ path: { uuid: 'srv-1' } });
    }
  );

  it.each([409, 500])('keeps the dialog open on %i', async (status) => {
    vi.mocked(deleteServer).mockResolvedValue(result(status, 'nope'));
    await mount();
    await confirmDelete();
    await vi.waitFor(() =>
      expect(sonner.error).toHaveBeenCalledWith('Failed to delete server')
    );
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(goto).not.toHaveBeenCalled();
  });
});

describe('server edit page tabs', () => {
  const tab = (name: string) =>
    screen.getByRole('tab', { name: new RegExp(`^${name}`) });
  const active = () =>
    screen.getAllByRole('tab').filter((t) => t.dataset.state === 'active');

  it.each([
    ['status', 'Status'],
    ['general', 'General'],
    ['startup', 'Startup'],
    ['variables', 'Variables'],
    ['resources', 'Resources'],
    ['permissions', 'Permissions']
  ])('opens the /%s param as %s', async (param, name) => {
    page.params = { tab: param, uuid: 'srv-1' };
    await mount();
    expect(tab(name)).toHaveAttribute('data-state', 'active');
    expect(active()).toHaveLength(1);
    expect(goto).not.toHaveBeenCalled();
  });

  it('defaults to Status without a tab param', async () => {
    page.params = { uuid: 'srv-1' };
    await mount();
    expect(tab('Status')).toHaveAttribute('data-state', 'active');
  });

  it('navigates to the tab URL keeping focus and scroll', async () => {
    await mount();
    await openTab('Startup');
    expect(goto).toHaveBeenCalledExactlyOnceWith('/servers/srv-1/startup', {
      keepFocus: true,
      noScroll: true,
      replaceState: false
    });
    expect(tab('Startup')).toHaveAttribute('data-state', 'active');
  });

  it('does not navigate when clicking the open tab', async () => {
    await mount();
    await openTab('Status');
    expect(goto).not.toHaveBeenCalled();
  });

  it('keeps unsaved edits when switching tabs', async () => {
    page.params = { tab: 'general', uuid: 'srv-1' };
    await mount();
    await fireEvent.input(screen.getByLabelText('Name'), {
      target: { value: 'Draft' }
    });
    expect(await screen.findByText('Unsaved changes')).toBeInTheDocument();
    await openTab('Status');
    await openTab('Resources');
    await openTab('General');
    expect(screen.getByLabelText('Name')).toHaveValue('Draft');
    expect(screen.getByText('Unsaved changes')).toBeInTheDocument();
  });

  it.each(['write', 'read'] as const)(
    'replaces /permissions with /status for %s access',
    async (access) => {
      page.params = { tab: 'permissions', uuid: 'srv-1' };
      await mount(asAccess(access));
      await vi.waitFor(() =>
        expect(goto).toHaveBeenCalledExactlyOnceWith('/servers/srv-1/status', {
          keepFocus: true,
          noScroll: true,
          replaceState: true
        })
      );
      expect(tab('Status')).toHaveAttribute('data-state', 'active');
    }
  );

  it('does not redirect /permissions before the server loads', async () => {
    page.params = { tab: 'permissions', uuid: 'srv-1' };
    render(EditPage, {
      props: {
        data: {
          options: new Promise(() => {}),
          serverRes: new Promise(() => {}),
          uuid: 'srv-1'
        }
      } as never
    });
    await tick(50);
    expect(goto).not.toHaveBeenCalled();
  });
});
