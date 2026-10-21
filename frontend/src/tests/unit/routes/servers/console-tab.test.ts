import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { flushSync } from 'svelte';
import { toast } from '@profidev/pleiades/components/util/general';
import { type ServerInfo, serverCommand, serverPower } from '$lib/client';
import ConsoleTab from '$routes/servers/[uuid]/ConsoleTab.svelte';
import { ServerLive } from '$routes/servers/[uuid]/live.svelte';
import { powerActions } from '$routes/servers/status';
import { FakeWebSocket, stubWebSocket } from '$test_helpers/fake-websocket';
import { serverInfo } from './fixtures';

vi.mock('@profidev/pleiades/components/util/general', async (orig) => ({
  ...(await orig<object>()),
  toast: { error: vi.fn(), success: vi.fn() }
}));

vi.mock('$lib/client', async (orig) => ({
  ...(await orig<object>()),
  serverCommand: vi.fn(),
  serverPower: vi.fn()
}));

const result = (status: number) =>
  ({
    data: status === 200 ? {} : undefined,
    error: status === 200 ? undefined : 'nope',
    request: new Request('http://x'),
    response: new Response(null, { status })
  }) as never;

type Status = ServerInfo['status'];

const server = (
  status: Status = 'running',
  over: Partial<ServerInfo> = {}
): ServerInfo => ({ ...structuredClone(serverInfo), status, ...over });

let live = new ServerLive('srv-1');

beforeEach(() => {
  stubWebSocket();
  live = new ServerLive('srv-1');
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

const mount = (info = server()) =>
  render(ConsoleTab, { props: { live, server: info } });

const button = (name: string) => screen.getByRole('button', { name });

const POWER = {
  kill: 'Kill',
  restart: 'Restart',
  start: 'Start',
  stop: 'Stop'
};

const stats = {
  cpu_limit_millicores: 2000,
  cpu_millicores: 760,
  disk_bytes: 3.4 * 1024 ** 3,
  disk_limit_bytes: 10 * 1024 ** 3,
  memory_bytes: 2.1 * 1024 ** 3,
  memory_limit_bytes: 4 * 1024 ** 3,
  net_rx_bps: 14_336,
  net_tx_bps: 3072,
  type: 'stats',
  uptime_ms: (3 * 24 * 60 + 4 * 60 + 12) * 60_000
} as const;

describe('ConsoleTab alerts', () => {
  it.each([
    ['install_failed', 'Install failed'],
    ['suspended', 'This server is suspended'],
    ['installing', 'The server is installing']
  ] as const)('shows the %s alert', (status, title) => {
    mount(server(status));
    expect(screen.getByRole('alert')).toHaveTextContent(title);
  });

  it('shows the node error in mono', () => {
    mount(server('offline', { error: 'Failed to start: no image' }));
    const alert = screen.getByRole('alert');
    expect(alert).toHaveTextContent('Node reported an error');
    expect(screen.getByText('Failed to start: no image')).toHaveClass(
      'font-mono'
    );
  });

  it.each(['offline', 'running'] as const)('has no alert while %s', (s) => {
    mount(server(s));
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});

describe('ConsoleTab server card', () => {
  it('shows the address, node and uptime', () => {
    mount();
    expect(screen.getByText('10.0.0.5:25565')).toHaveClass('font-mono');
    expect(button('Copy address')).toBeInTheDocument();
    expect(screen.getByText('eu-1')).toBeInTheDocument();
    expect(screen.getByText('Uptime').nextElementSibling).toHaveTextContent(
      '—'
    );
    live.handle(stats);
    flushSync();
    expect(screen.getByText('Uptime').nextElementSibling).toHaveTextContent(
      '3d 4h 12m'
    );
  });

  it('dashes the uptime when not running', () => {
    mount(server('offline'));
    live.handle(stats);
    flushSync();
    expect(screen.getByText('Uptime').nextElementSibling).toHaveTextContent(
      '—'
    );
  });
});

describe('ConsoleTab power', () => {
  it.each([
    'running',
    'starting',
    'stopping',
    'offline',
    'crashed',
    'installing',
    'suspended',
    'install_failed'
  ] as const)('follows the power matrix while %s', (status) => {
    mount(server(status));
    const allowed = powerActions(status);
    for (const [action, name] of Object.entries(POWER)) {
      if (allowed.includes(action as never)) {
        expect(button(name)).toBeEnabled();
      } else {
        expect(button(name)).toBeDisabled();
      }
    }
  });

  it.each([
    ['running', 'Kill ends the process'],
    ['offline', 'Start the server to use the console.'],
    ['installing', 'Power actions are locked.']
  ] as const)('explains power while %s', (status, hint) => {
    mount(server(status));
    expect(screen.getByText(new RegExp(hint))).toBeInTheDocument();
  });

  it('hides power for read access', () => {
    mount(server('running', { access: 'read' }));
    expect(screen.queryByText('Power')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Kill' })
    ).not.toBeInTheDocument();
  });

  it('sends the action', async () => {
    vi.mocked(serverPower).mockResolvedValue(result(200));
    mount(server('running'));
    await fireEvent.click(button('Restart'));
    expect(serverPower).toHaveBeenCalledWith({
      body: { action: 'restart' },
      path: { uuid: 'srv-1' }
    });
    await vi.waitFor(() => expect(button('Restart')).toBeEnabled());
    expect(toast.error).not.toHaveBeenCalled();
  });

  it.each([
    [409, "The server can't do that right now."],
    [500, "Couldn't send the power action."]
  ])('toasts a %i', async (status, message) => {
    vi.mocked(serverPower).mockResolvedValue(result(status));
    mount(server('offline'));
    await fireEvent.click(button('Start'));
    await vi.waitFor(() => expect(toast.error).toHaveBeenCalledWith(message));
  });
});

describe('ConsoleTab console', () => {
  const input = () => screen.getByLabelText('Console command');

  it('shows the connection badge', () => {
    mount(server('running'));
    expect(screen.getByText('Disconnected')).toBeInTheDocument();
    live.connect();
    FakeWebSocket.last!.open();
    flushSync();
    expect(screen.getByText('Connected')).toBeInTheDocument();
    live.disconnect();
  });

  it('labels the badge Install log while installing', () => {
    mount(server('installing'));
    live.connect();
    FakeWebSocket.last!.open();
    flushSync();
    expect(screen.getByText('Install log')).toBeInTheDocument();
    live.disconnect();
  });

  it('renders lines with time, daemon prefix and colours', () => {
    mount();
    live.handle({
      lines: [
        {
          at: new Date(2026, 0, 1, 12, 4, 1),
          line: 'Server marked as starting...',
          stream: 'daemon'
        },
        {
          at: new Date(2026, 0, 1, 12, 4, 2),
          line: "[Server thread/WARN]: Can't keep up!",
          stream: 'console'
        },
        {
          at: new Date(2026, 0, 1, 12, 4, 3),
          line: 'NullPointerException',
          stream: 'console'
        },
        { at: new Date(2026, 0, 1, 12, 4, 4), line: 'Done', stream: 'install' },
        {
          at: new Date(2026, 0, 1, 12, 4, 5),
          line: '\u001B[31mred\u001B[0m WARN plain\u001B[2K',
          stream: 'console'
        }
      ],
      type: 'logs'
    });
    flushSync();
    const log = screen.getByRole('log');
    expect(log).toHaveAttribute('aria-live', 'off');
    expect(log).toHaveTextContent('12:04:01');
    // Segments are spans; the line colour sits on their parent.
    const line = (text: string | RegExp) =>
      screen.getByText(text).parentElement!;
    const daemon = line('Server marked as starting...');
    expect(daemon).toHaveTextContent('[Daemon] Server marked as starting...');
    expect(daemon.className).toMatch(/oklch\(0\.7_/);
    expect(line(/Can't keep up/).className).toMatch(/oklch\(0\.8_/);
    expect(line('NullPointerException').className).toMatch(/oklch\(0\.65_/);
    expect(line('Done').className).not.toMatch(/oklch/);
    // ANSI colour overrides the WARN base colour on its segment only.
    const red = screen.getByText('red');
    expect(red).toHaveStyle({ color: 'var(--ansi-1)' });
    expect(red.parentElement).toHaveTextContent(/^red WARN plain$/);
    expect(red.parentElement!.className).toMatch(/oklch\(0\.8_/);
  });

  it('sends a command on Enter and echoes it muted', async () => {
    vi.mocked(serverCommand).mockResolvedValue(result(200));
    mount();
    await fireEvent.input(input(), { target: { value: '  say hi ' } });
    await fireEvent.submit(input().closest('form')!);
    expect(serverCommand).toHaveBeenCalledWith({
      body: { line: 'say hi' },
      path: { uuid: 'srv-1' }
    });
    expect(input()).toHaveValue('');
    const echoed = screen.getByText('say hi').parentElement!;
    expect(echoed).toHaveTextContent('> say hi');
    expect(echoed).toHaveClass('text-muted-foreground');
  });

  it('ignores blank commands', async () => {
    mount();
    await fireEvent.input(input(), { target: { value: '   ' } });
    await fireEvent.submit(input().closest('form')!);
    expect(serverCommand).not.toHaveBeenCalled();
  });

  it('toasts a failed command', async () => {
    vi.mocked(serverCommand).mockResolvedValue(result(409));
    mount();
    await fireEvent.input(input(), { target: { value: 'list' } });
    await fireEvent.click(button('Send command'));
    await vi.waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith('The server is not running.')
    );
  });

  it.each([
    ['installing', 'admin', 'Commands are unavailable during install'],
    ['starting', 'admin', 'Waiting for the server to start'],
    ['offline', 'admin', 'Server must be running'],
    ['running', 'read', 'Commands need write access']
  ] as const)(
    'disables the input while %s with %s access',
    (status, access, placeholder) => {
      mount(server(status, { access }));
      expect(input()).toBeDisabled();
      expect(input()).toHaveAttribute('placeholder', placeholder);
      expect(button('Send command')).toBeDisabled();
    }
  );

  it('enables the input while running with write access', () => {
    mount(server('running', { access: 'write' }));
    expect(input()).toBeEnabled();
    expect(input()).toHaveAttribute('placeholder', 'Type a command');
  });
});

describe('ConsoleTab metrics', () => {
  it('shows zeros before the first stats frame', () => {
    mount(server('offline'));
    expect(screen.getByText('0m')).toBeInTheDocument();
    expect(screen.getAllByText('Server is offline')).toHaveLength(3);
    expect(screen.getByText('Unlimited')).toBeInTheDocument();
  });

  it('shows the latest numbers in the card headers', () => {
    mount(server('running'));
    live.handle(stats);
    flushSync();
    expect(screen.getByText('760m')).toBeInTheDocument();
    expect(screen.getByText('38% of 2000 millicores')).toBeInTheDocument();
    expect(screen.getByText('2.1 GiB')).toBeInTheDocument();
    expect(screen.getByText('53% of 4 GiB limit')).toBeInTheDocument();
    expect(screen.getByText('↓ 14 KB/s')).toBeInTheDocument();
    expect(screen.getByText('Outbound ↑ 3.1 KB/s')).toBeInTheDocument();
    expect(screen.getByText('3.4 GiB')).toBeInTheDocument();
    expect(screen.getByText('34% used')).toBeInTheDocument();
    expect(screen.getByText('of 10 GiB')).toBeInTheDocument();
    for (const name of ['CPU', 'Memory', 'Network']) {
      expect(
        screen.getByRole('img', { name: `${name} over the last 60 seconds` })
      ).toBeInTheDocument();
    }
  });

  it('labels unlimited CPU and memory', () => {
    mount(server('running'));
    live.handle({ ...stats, cpu_limit_millicores: 0, memory_limit_bytes: 0 });
    flushSync();
    expect(screen.getByText('No CPU limit')).toBeInTheDocument();
    expect(screen.getByText('No memory limit')).toBeInTheDocument();
  });
});
