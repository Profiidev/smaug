import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within
} from '@testing-library/svelte';
import { toast as sonner } from 'svelte-sonner';
import { invalidate } from '$app/navigation';
import { toast } from '@profidev/pleiades/components/util/general';
import {
  type ServerInfo,
  reinstallServer,
  setServerInstalled,
  setServerSuspended
} from '$lib/client';
import StatusTab from '$routes/servers/[uuid]/StatusTab.svelte';
import { availableActions } from '$routes/servers/status';
import { options, serverInfo } from './fixtures';

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
  reinstallServer: vi.fn(),
  setServerInstalled: vi.fn(),
  setServerSuspended: vi.fn()
}));

afterEach(cleanup);

beforeEach(() => {
  vi.spyOn(sonner, 'error').mockImplementation(() => 0);
});

const tick = async (ms = 0) => new Promise((r) => setTimeout(r, ms));

const result = (status: number) =>
  ({
    data: status === 200 ? {} : undefined,
    error: status === 200 ? undefined : 'nope',
    request: new Request('http://x'),
    response: new Response(null, { status })
  }) as never;

type Status = ServerInfo['status'];

const STATES: Record<
  Status,
  Pick<ServerInfo, 'install_state' | 'suspended'>
> = {
  crashed: { install_state: 'installed', suspended: false },
  install_failed: { install_state: 'install_failed', suspended: false },
  installing: { install_state: 'installing', suspended: false },
  not_installed: { install_state: 'not_installed', suspended: false },
  offline: { install_state: 'installed', suspended: false },
  running: { install_state: 'installed', suspended: false },
  starting: { install_state: 'installed', suspended: false },
  stopping: { install_state: 'installed', suspended: false },
  suspended: { install_state: 'installed', suspended: true },
  unknown: { install_state: 'installed', suspended: false }
};

const server = (
  status: Status = 'offline',
  access: ServerInfo['access'] = 'admin'
): ServerInfo => ({
  ...structuredClone(serverInfo),
  ...STATES[status],
  access,
  status
});

const mount = (
  info = server(),
  { nodes = options.nodes, ondelete = vi.fn() } = {}
) => {
  render(StatusTab, { props: { nodes, ondelete, server: info } });
  return { ondelete };
};

const button = (name: string | RegExp) => screen.getByRole('button', { name });

const BUTTONS = {
  delete: 'Delete server',
  install: /^Mark as/,
  reinstall: 'Reinstall',
  suspend: /^(?<verb>Suspend|Unsuspend)$/,
  transfer: 'Transfer…'
} as const;

describe('StatusTab cards', () => {
  it('shows the status, usage placeholder and details', () => {
    mount();
    expect(
      screen.getByText('The server is installed and stopped.')
    ).toBeInTheDocument();
    expect(screen.getByText('Offline')).toBeInTheDocument();
    expect(screen.getByText(/^Created /)).toBeInTheDocument();
    expect(
      screen.getByText('Usage is shown while the server is running.')
    ).toBeInTheDocument();
    const details = screen.getByText('Primary allocation').closest('dl')!;
    const value = (label: string) =>
      within(details).getByText(label).nextElementSibling!;
    expect(value('Name')).toHaveTextContent('survival');
    expect(value('Node')).toHaveTextContent('eu-1');
    expect(value('Primary allocation')).toHaveTextContent('10.0.0.5:25565');
    expect(value('Extra allocations')).toHaveTextContent('10.0.0.5:25566');
    expect(value('Egg')).toHaveTextContent('Paper');
    expect(value('Image')).toHaveTextContent('ghcr.io/yolks:java_21');
    expect(value('UUID')).toHaveTextContent('srv-1');
    expect(value('UUID')).toHaveClass('font-mono');
    expect(value('Egg')).not.toHaveClass('font-mono');
  });

  it('shows the creation date', () => {
    mount(server());
    expect(screen.getByText(/^Created /)).toHaveTextContent(
      `Created ${serverInfo.created_at.toLocaleDateString()}`
    );
  });

  it('brackets IPv6 and dashes missing allocations', () => {
    mount({ ...server(), allocations: [{ ip: '::1', port: 30_000 }] });
    expect(screen.getByText('[::1]:30000')).toBeInTheDocument();
    expect(
      screen.getByText('Extra allocations').nextElementSibling
    ).toHaveTextContent('—');
  });

  it('notes that install progress comes from the node', () => {
    mount(server('installing'));
    expect(
      screen.getByText(
        'Install progress appears here once the node reports it.'
      )
    ).toBeInTheDocument();
  });

  it('shows a destructive alert when the install failed', () => {
    mount(server('install_failed'));
    expect(screen.getByRole('alert')).toHaveTextContent('Install failed');
  });

  it('shows an alert when suspended', () => {
    mount(server('suspended'));
    expect(screen.getByRole('alert')).toHaveTextContent('Suspended');
  });

  it.each(['offline', 'not_installed'] as const)(
    'shows no alert while %s',
    (status) => {
      mount(server(status));
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    }
  );
});

describe('StatusTab action availability', () => {
  it.each(Object.keys(STATES) as Status[])(
    'admin while %s follows the matrix',
    (status) => {
      mount(server(status));
      const allowed = availableActions(status);
      for (const [action, name] of Object.entries(BUTTONS)) {
        const expected = allowed[action as keyof typeof allowed];
        if (expected) {
          expect(button(name)).toBeEnabled();
        } else {
          expect(button(name)).toBeDisabled();
        }
      }
    }
  );

  it.each(Object.keys(STATES) as Status[])(
    'write access while %s may only reinstall',
    (status) => {
      mount(server(status, 'write'));
      if (availableActions(status).reinstall) {
        expect(button('Reinstall')).toBeEnabled();
      } else {
        expect(button('Reinstall')).toBeDisabled();
      }
      for (const name of [
        BUTTONS.install,
        BUTTONS.suspend,
        BUTTONS.transfer,
        BUTTONS.delete
      ]) {
        expect(button(name)).toBeDisabled();
      }
      expect(
        screen.getByText(/These change the server's state/)
      ).toBeInTheDocument();
    }
  );

  it('read access disables every action and says why', () => {
    mount(server('offline', 'read'));
    for (const name of Object.values(BUTTONS)) {
      expect(button(name)).toBeDisabled();
    }
    expect(
      screen.getByText(
        'You have view-only access. Actions need write access to this server.'
      )
    ).toBeInTheDocument();
  });

  it('labels the install toggle by install state', () => {
    mount(server('offline'));
    expect(button('Mark as not installed')).toBeInTheDocument();
    cleanup();
    mount(server('not_installed'));
    expect(button('Mark as installed')).toBeInTheDocument();
  });

  it('swaps Suspend for Unsuspend while suspended', () => {
    mount(server('suspended'));
    expect(button('Unsuspend')).toBeEnabled();
    expect(
      screen.queryByRole('button', { name: 'Suspend' })
    ).not.toBeInTheDocument();
  });

  it('disables transfer without another node', () => {
    mount(server(), { nodes: [options.nodes[0]] });
    expect(button('Transfer…')).toBeDisabled();
  });
});

describe('StatusTab actions', () => {
  const expectRefreshed = () => {
    // Only this server's info: the list and options would reset nothing useful.
    expect(vi.mocked(invalidate).mock.calls).toEqual([['/api/servers/srv-1']]);
  };

  const confirm = async (open: string, title: string, action: string) => {
    await fireEvent.click(button(open));
    expect(await screen.findByText(title)).toBeInTheDocument();
    await fireEvent.click(
      within(screen.getByRole('dialog')).getByRole('button', { name: action })
    );
    await tick(50);
  };

  it('reinstalls after confirming', async () => {
    vi.mocked(reinstallServer).mockResolvedValue(result(200));
    mount();
    await confirm('Reinstall', 'Reinstall survival?', 'Reinstall');
    await vi.waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith('Reinstalling survival')
    );
    expect(reinstallServer).toHaveBeenCalledWith({ path: { uuid: 'srv-1' } });
    expectRefreshed();
    await vi.waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
  });

  it('keeps the reinstall dialog open on failure', async () => {
    vi.mocked(reinstallServer).mockResolvedValue(result(409));
    mount();
    await confirm('Reinstall', 'Reinstall survival?', 'Reinstall');
    await vi.waitFor(() =>
      expect(sonner.error).toHaveBeenCalledWith("Couldn't reinstall server.")
    );
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(invalidate).not.toHaveBeenCalled();
  });

  it.each([
    ['offline', false, 'survival marked as not installed'],
    ['installing', true, 'survival marked as installed']
  ] as const)(
    'toggles the install state while %s without confirming',
    async (status, installed, message) => {
      vi.mocked(setServerInstalled).mockResolvedValue(result(200));
      mount(server(status));
      await fireEvent.click(button(BUTTONS.install));
      await vi.waitFor(() =>
        expect(toast.success).toHaveBeenCalledWith(message)
      );
      expect(setServerInstalled).toHaveBeenCalledWith({
        body: { installed },
        path: { uuid: 'srv-1' }
      });
      expectRefreshed();
    }
  );

  it('toasts a failed install toggle', async () => {
    vi.mocked(setServerInstalled).mockResolvedValue(result(409));
    mount();
    await fireEvent.click(button(BUTTONS.install));
    await vi.waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith(
        "Couldn't change the install status."
      )
    );
    expect(invalidate).not.toHaveBeenCalled();
    expect(button(BUTTONS.install)).toBeEnabled();
  });

  it('suspends after confirming', async () => {
    vi.mocked(setServerSuspended).mockResolvedValue(result(200));
    mount();
    await confirm('Suspend', 'Suspend survival?', 'Suspend');
    await vi.waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith('survival suspended')
    );
    expect(setServerSuspended).toHaveBeenCalledWith({
      body: { suspended: true },
      path: { uuid: 'srv-1' }
    });
    expectRefreshed();
  });

  it('keeps the suspend dialog open on failure', async () => {
    vi.mocked(setServerSuspended).mockResolvedValue(result(500));
    mount();
    await confirm('Suspend', 'Suspend survival?', 'Suspend');
    await vi.waitFor(() =>
      expect(sonner.error).toHaveBeenCalledWith("Couldn't suspend server.")
    );
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });

  it('unsuspends without confirming', async () => {
    vi.mocked(setServerSuspended).mockResolvedValue(result(200));
    mount(server('suspended'));
    await fireEvent.click(button('Unsuspend'));
    await vi.waitFor(() =>
      expect(toast.success).toHaveBeenCalledWith('survival unsuspended')
    );
    expect(setServerSuspended).toHaveBeenCalledWith({
      body: { suspended: false },
      path: { uuid: 'srv-1' }
    });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('toasts a failed unsuspend', async () => {
    vi.mocked(setServerSuspended).mockResolvedValue(result(409));
    mount(server('suspended'));
    await fireEvent.click(button('Unsuspend'));
    await vi.waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith("Couldn't unsuspend server.")
    );
    expect(invalidate).not.toHaveBeenCalled();
  });

  it('opens the transfer dialog', async () => {
    mount();
    await fireEvent.click(button('Transfer…'));
    expect(await screen.findByText('Transfer survival')).toBeInTheDocument();
  });

  it('asks the page to delete', async () => {
    const { ondelete } = mount();
    await fireEvent.click(button('Delete server'));
    expect(ondelete).toHaveBeenCalledOnce();
  });
});
