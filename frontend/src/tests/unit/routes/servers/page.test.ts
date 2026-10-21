import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within
} from '@testing-library/svelte';
import { toast as sonner } from 'svelte-sonner';
import { goto, invalidate } from '$app/navigation';
import { toast } from '@profidev/pleiades/components/util/general';
import { createServer, deleteServer } from '$lib/client';
import ListPage from '$routes/servers/+page.svelte';
import CreatePage from '$routes/servers/create/+page.svelte';
import { adminUser } from '$mocks/e2e/data';
import { Permission } from '$lib/permissions.svelte';
import { options, servers } from './fixtures';

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
  createServer: vi.fn(),
  deleteServer: vi.fn()
}));

afterEach(cleanup);

// BaseForm / FormDialog report submit errors through svelte-sonner directly.
beforeEach(() => {
  vi.spyOn(sonner, 'error').mockImplementation(() => 0);
});

const tick = async (ms = 0) => new Promise((r) => setTimeout(r, ms));

/** The bits-ui Select: opens from the keyboard (jsdom can't drive pointerdown), items select on pointerup. */
const chooseStatus = async (name: string) => {
  await fireEvent.keyDown(screen.getByLabelText('Filter by status'), {
    key: 'ArrowDown'
  });
  await fireEvent.pointerUp(await screen.findByRole('option', { name }));
  await tick();
};

const result = (status: number, body: unknown) =>
  ({
    data: status === 200 ? body : undefined,
    error: status === 200 ? undefined : body,
    request: new Request('http://x'),
    response: new Response(null, { status })
  }) as never;

const readonlyUser = {
  ...adminUser,
  permissions: adminUser.permissions.filter((p) => p !== Permission.SERVER_EDIT)
};

const listData = (
  list: unknown[] | undefined,
  user = adminUser,
  error: string | null = null
) => ({
  error,
  servers: Promise.resolve(list),
  user: Promise.resolve(user)
});

const renderList = (data = listData(servers)) =>
  render(ListPage, { props: { data } as never });

describe('servers list page', () => {
  it('shows the header, rows and count', async () => {
    renderList();
    expect(
      screen.getByRole('heading', { name: 'Servers' })
    ).toBeInTheDocument();
    expect(await screen.findByText('2 servers')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'survival' })).toHaveAttribute(
      'href',
      '/servers/srv-1/status'
    );
    // Container matches the nav; Server is the nodes list's icon.
    const row = screen.getByRole('link', { name: 'survival' }).closest('tr')!;
    expect(row.querySelector('svg.lucide-container')).not.toBeNull();
    expect(row.querySelector('svg.lucide-server')).toBeNull();
    expect(screen.getByText('10.0.0.5:25565 +1')).toBeInTheDocument();
    expect(screen.getByText('[::1]:30000')).toBeInTheDocument();
    const table = screen.getByRole('table');
    expect(within(table).getByText('Installing')).toBeInTheDocument();
    expect(within(table).getByText('Offline')).toBeInTheDocument();
  });

  it('uses the singular for one server', async () => {
    renderList(listData(servers.slice(0, 1)));
    expect(await screen.findByText('1 server')).toBeInTheDocument();
  });

  it('shows no count while loading', () => {
    render(ListPage, {
      props: {
        data: {
          error: null,
          servers: new Promise(() => {}),
          user: Promise.resolve(adminUser)
        }
      } as never
    });
    expect(screen.queryByText(/\d+ servers?$/)).not.toBeInTheDocument();
    expect(screen.queryByText('No servers yet')).not.toBeInTheDocument();
    expect(screen.getByLabelText('Filter by status')).toBeDisabled();
  });

  it.each([[[]], [undefined]])(
    'shows the empty state for %j with one create button and no table',
    async (list) => {
      renderList(listData(list));
      expect(await screen.findByText('No servers yet')).toBeInTheDocument();
      expect(screen.getAllByRole('link', { name: 'Create' })).toHaveLength(1);
      expect(screen.queryByRole('table')).not.toBeInTheDocument();
      expect(
        screen.queryByLabelText('Filter by status')
      ).not.toBeInTheDocument();
    }
  );

  it('filters by status', async () => {
    renderList();
    await screen.findByText('2 servers');
    await chooseStatus('Installing');
    expect(screen.getByLabelText('Filter by status')).toHaveTextContent(
      'Installing'
    );
    expect(await screen.findByText('1 server')).toBeInTheDocument();
    expect(screen.queryByRole('link', { name: 'survival' })).toBeNull();
    expect(screen.getByRole('link', { name: 'creative' })).toBeInTheDocument();
    await chooseStatus('Suspended');
    expect(await screen.findByText('0 servers')).toBeInTheDocument();
    await chooseStatus('All statuses');
    expect(await screen.findByText('2 servers')).toBeInTheDocument();
  });

  it('searches by node and allocation', async () => {
    renderList();
    await screen.findByText('2 servers');
    const search = screen.getByPlaceholderText('Search...');
    await fireEvent.input(search, { target: { value: 'us-1' } });
    expect(screen.queryByRole('link', { name: 'survival' })).toBeNull();
    await fireEvent.input(search, { target: { value: '25565' } });
    expect(screen.getByRole('link', { name: 'survival' })).toBeInTheDocument();
    expect(screen.queryByRole('link', { name: 'creative' })).toBeNull();
  });

  it('combines search with the status filter in the count', async () => {
    renderList();
    await screen.findByText('2 servers');
    const search = screen.getByPlaceholderText('Search...');
    // Search and status filter share one toolbar row.
    expect(search.parentElement).toBe(
      screen.getByLabelText('Filter by status').parentElement
    );
    await fireEvent.input(search, { target: { value: 'survival' } });
    expect(await screen.findByText('1 server')).toBeInTheDocument();
    await chooseStatus('Installing');
    expect(await screen.findByText('0 servers')).toBeInTheDocument();
    await fireEvent.input(search, { target: { value: '' } });
    expect(await screen.findByText('1 server')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'creative' })).toBeInTheDocument();
  });

  it('enables create for server:edit', async () => {
    renderList();
    await screen.findByText('2 servers');
    await vi.waitFor(() =>
      expect(screen.getByRole('link', { name: 'Create' })).not.toHaveAttribute(
        'aria-disabled',
        'true'
      )
    );
  });

  it('disables create, edit and delete without server:edit', async () => {
    renderList(listData(servers, readonlyUser));
    await screen.findByText('2 servers');
    await vi.waitFor(() =>
      expect(screen.getByRole('link', { name: 'Create' })).toHaveAttribute(
        'aria-disabled',
        'true'
      )
    );
    const row = screen.getByRole('link', { name: 'survival' }).closest('tr')!;
    for (const b of row.querySelectorAll('button')) {
      expect(b).toBeDisabled();
    }
  });

  it.each([
    ['not_found', 'Server not found'],
    ['other', 'Failed to load server'],
    ['options', 'Failed to load server options']
  ])('toasts ?error=%s and strips it from the URL', async (error, message) => {
    window.history.replaceState({}, '', `/servers?error=${error}&x=1`);
    renderList(listData(servers, adminUser, error));
    await vi.waitFor(() => expect(toast.error).toHaveBeenCalledWith(message));
    expect(window.location.search).toBe('?x=1');
  });

  it('strips an unknown error without a toast', async () => {
    window.history.replaceState({}, '', '/servers?error=weird');
    renderList(listData(servers, adminUser, 'weird'));
    await vi.waitFor(() => expect(window.location.search).toBe(''));
    expect(toast.error).not.toHaveBeenCalled();
  });

  describe('delete', () => {
    const openDelete = async () => {
      renderList();
      await screen.findByText('2 servers');
      const row = screen.getByRole('link', { name: 'survival' }).closest('tr')!;
      const buttons = row.querySelectorAll('button');
      await vi.waitFor(() =>
        expect(buttons[buttons.length - 1]).not.toBeDisabled()
      );
      await fireEvent.click(buttons[buttons.length - 1]);
      expect(
        await screen.findByText('Delete server survival?')
      ).toBeInTheDocument();
    };

    it('deletes, toasts and invalidates on success', async () => {
      vi.mocked(deleteServer).mockResolvedValue(result(200, undefined));
      await openDelete();
      await fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
      await vi.waitFor(() =>
        expect(toast.success).toHaveBeenCalledWith(
          'Server survival deleted successfully'
        )
      );
      expect(deleteServer).toHaveBeenCalledWith({ path: { uuid: 'srv-1' } });
      const pred = vi.mocked(invalidate).mock.calls[0][0] as (
        u: URL
      ) => boolean;
      expect(pred(new URL('http://x/api/servers'))).toBe(true);
      expect(pred(new URL('http://x/api/eggs'))).toBe(false);
    });

    it.each([409, 500])('shows an error on %i', async (status) => {
      vi.mocked(deleteServer).mockResolvedValue(result(status, ''));
      await openDelete();
      await fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
      await vi.waitFor(() =>
        expect(sonner.error).toHaveBeenCalledWith('Failed to delete server')
      );
      expect(screen.getByText('Delete server survival?')).toBeInTheDocument();
      expect(toast.success).not.toHaveBeenCalled();
      expect(invalidate).not.toHaveBeenCalled();
    });
  });
});

describe('server create page', () => {
  const createData = (opts: unknown, user = adminUser) => ({
    options: Promise.resolve(opts),
    user: Promise.resolve(user)
  });

  const renderCreate = (data = createData(options)) =>
    render(CreatePage, { props: { data } as never });

  const pick = async (current: string, option: string) => {
    const trigger = screen
      .getAllByRole('combobox')
      .find((b) => b.textContent?.trim() === current)!;
    await fireEvent.click(trigger);
    await fireEvent.click(await screen.findByRole('option', { name: option }));
    await tick();
  };

  const next = async () => {
    await fireEvent.click(screen.getByRole('button', { name: /^Next/ }));
    await tick(50);
  };

  /** Fills General and walks to the Resources stage. */
  const fillWizard = async () => {
    renderCreate();
    await fireEvent.input(await screen.findByLabelText('Name'), {
      target: { value: 'survival' }
    });
    await pick('No Node', 'eu-1');
    await pick('No Egg', 'Paper');
    await fireEvent.input(screen.getByLabelText('IP address'), {
      target: { value: '10.0.0.5' }
    });
    await next();
    await screen.findByLabelText('Startup command');
    await next();
    await screen.findByLabelText('Server Jar File');
    await next();
    await screen.findByRole('switch', { name: 'Unlimited CPU' });
  };

  const submitWizard = async () => {
    await fireEvent.click(screen.getByRole('button', { name: 'Create' }));
    await tick(50);
  };

  it('shows a skeleton until the options load', () => {
    render(CreatePage, {
      props: {
        data: {
          options: new Promise(() => {}),
          user: Promise.resolve(adminUser)
        }
      } as never
    });
    expect(
      screen.getByRole('heading', { name: 'Create server' })
    ).toBeInTheDocument();
    expect(screen.queryByLabelText('Name')).not.toBeInTheDocument();
  });

  it('redirects to the list when the options fail', async () => {
    renderCreate(createData(undefined));
    await vi.waitFor(() =>
      expect(goto).toHaveBeenCalledWith('/servers?error=options')
    );
  });

  it('defaults the owner to the current user', async () => {
    renderCreate();
    await screen.findByLabelText('Name');
    expect(
      screen.getAllByRole('combobox').map((b) => b.textContent?.trim())
    ).toEqual(['Ada Admin', 'No Node', 'No Egg']);
  });

  it('leaves the owner empty when the user is not an option', async () => {
    renderCreate(createData(options, { ...adminUser, uuid: 'someone' }));
    await screen.findByLabelText('Name');
    expect(screen.getAllByRole('combobox')[0]).toHaveTextContent('No Owner');
  });

  it('warns when there is no node or egg', async () => {
    renderCreate(createData({ ...options, nodes: [] }));
    expect(
      await screen.findByText('Nothing to create a server on yet')
    ).toBeInTheDocument();
  });

  it('does not warn with nodes and eggs', async () => {
    renderCreate();
    await screen.findByLabelText('Name');
    expect(
      screen.queryByText('Nothing to create a server on yet')
    ).not.toBeInTheDocument();
  });

  it('stays on General while it is invalid', async () => {
    renderCreate();
    await screen.findByLabelText('Name');
    await next();
    await tick(150);
    expect(screen.getByLabelText('Name')).toBeInTheDocument();
    expect(screen.queryByLabelText('Startup command')).not.toBeInTheDocument();
  });

  it('loads startup and variables from the picked egg', async () => {
    await fillWizard();
    await fireEvent.click(screen.getByRole('button', { name: /Previous/ }));
    await tick(50);
    expect(screen.getByLabelText('Server Jar File')).toHaveValue('server.jar');
    await fireEvent.click(screen.getByRole('button', { name: /Previous/ }));
    await tick(50);
    expect(screen.getByLabelText('Startup command').textContent?.trim()).toBe(
      'Default'
    );
    expect(screen.getByLabelText('Command')).toHaveValue(
      'java -jar {{SERVER_JARFILE}}'
    );
    expect(screen.getByLabelText('Image name').textContent?.trim()).toBe(
      'Java 17'
    );
  });

  it('reloads the egg stages after confirming an egg change', async () => {
    await fillWizard();
    await fireEvent.click(screen.getByRole('button', { name: /Previous/ }));
    await tick(50);
    await fireEvent.input(screen.getByLabelText('Server Jar File'), {
      target: { value: 'edited.jar' }
    });
    // Back to General (via Variables -> Startup -> General).
    await fireEvent.click(screen.getByRole('button', { name: /Previous/ }));
    await tick(50);
    await fireEvent.click(screen.getByRole('button', { name: /Previous/ }));
    await tick(50);
    await pick('Paper', 'Vanilla');
    await fireEvent.click(
      await screen.findByRole('button', { name: 'Change egg' })
    );
    await tick();
    await next();
    expect(screen.getByLabelText('Command')).toHaveValue('./run.sh');
    expect(screen.getByLabelText('Image name').textContent?.trim()).toBe(
      'Alpine'
    );
    await next();
    expect(screen.getByText('No variables')).toBeInTheDocument();
  });

  it('creates the server, toasts and opens it', async () => {
    vi.mocked(createServer).mockResolvedValue(result(200, { uuid: 'srv-9' }));
    await fillWizard();
    await submitWizard();
    await vi.waitFor(() =>
      expect(goto).toHaveBeenCalledWith('/servers/srv-9/status')
    );
    expect(toast.success).toHaveBeenCalledWith('Server created successfully.');
    expect(createServer).toHaveBeenCalledWith({
      body: {
        allocations: [{ ip: '10.0.0.5', port: 25_565 }],
        backup_limit: 0,
        egg: 'egg-paper',
        environment: { MAX_PLAYERS: '20', SERVER_JARFILE: 'server.jar' },
        image: 'ghcr.io/yolks:java_17',
        limits: { cpu: 0, disk: 0, memory: 0 },
        name: 'survival',
        node: 'node-1',
        owner: 'user-admin',
        restart_on_crash: true,
        skip_scripts: false,
        start_on_completion: true,
        startup: 'java -jar {{SERVER_JARFILE}}',
        startup_overridden: false,
        stop_timeout: 60
      }
    });
  });

  it('jumps to the first stage with 400 field errors and shows them', async () => {
    vi.mocked(createServer).mockResolvedValue(
      result(400, {
        errors: [
          { field: 'limits.cpu', message: 'CPU too high.' },
          { field: 'name', message: 'Name is taken.' }
        ]
      })
    );
    await fillWizard();
    await submitWizard();
    await vi.waitFor(() =>
      expect(sonner.error).toHaveBeenCalledWith(
        "Couldn't create server. Fix the highlighted fields."
      )
    );
    expect(await screen.findByText('Name is taken.')).toBeInTheDocument();
    expect(screen.getByLabelText('Name')).toHaveValue('survival');
    expect(goto).not.toHaveBeenCalled();
  });

  it('shows a 409 allocation conflict on its row', async () => {
    vi.mocked(createServer).mockResolvedValue(
      result(409, {
        errors: [{ field: 'allocations.0', message: 'Allocation in use.' }]
      })
    );
    await fillWizard();
    await submitWizard();
    await vi.waitFor(() =>
      expect(sonner.error).toHaveBeenCalledWith(
        "Couldn't create server: an allocation is already in use."
      )
    );
    expect(await screen.findByText('Allocation in use.')).toBeInTheDocument();
  });

  it('toasts unmappable field errors and stays on the last stage', async () => {
    vi.mocked(createServer).mockResolvedValue(
      result(400, { errors: [{ field: 'mystery', message: 'Odd.' }] })
    );
    await fillWizard();
    await submitWizard();
    await vi.waitFor(() => expect(sonner.error).toHaveBeenCalledWith('Odd.'));
    expect(
      screen.getByRole('switch', { name: 'Unlimited CPU' })
    ).toBeInTheDocument();
  });

  it.each([403, 500])('toasts a generic error on %i', async (status) => {
    vi.mocked(createServer).mockResolvedValue(result(status, ''));
    await fillWizard();
    await submitWizard();
    await vi.waitFor(() =>
      expect(sonner.error).toHaveBeenCalledWith('Error creating server.')
    );
    expect(goto).not.toHaveBeenCalled();
  });
});
