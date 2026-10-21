import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/svelte';
import StatusBadge from '$routes/servers/StatusBadge.svelte';
import {
  STATUS,
  type ServerStatus,
  availableActions,
  powerActions
} from '$routes/servers/status';

afterEach(cleanup);

const statuses = Object.keys(STATUS) as ServerStatus[];

describe('STATUS map', () => {
  it('covers every server status', () => {
    expect(statuses.toSorted()).toEqual([
      'crashed',
      'install_failed',
      'installing',
      'not_installed',
      'offline',
      'running',
      'starting',
      'stopping',
      'suspended',
      'transferring',
      'unknown'
    ]);
  });

  it('uses the handoff labels and oklch dot colours', () => {
    expect(STATUS.running).toEqual({
      color: 'oklch(0.72 0.17 150)',
      label: 'Running'
    });
    expect(STATUS.install_failed.label).toBe('Install failed');
    expect(STATUS.not_installed).toEqual(
      expect.objectContaining({
        color: STATUS.offline.color,
        label: 'Not installed'
      })
    );
    for (const { color } of Object.values(STATUS)) {
      expect(color).toMatch(/^oklch\(/);
    }
  });
});

describe('StatusBadge', () => {
  it.each(statuses)('renders the label and dot for %s', (status) => {
    const { container } = render(StatusBadge, { props: { status } });
    expect(screen.getByText(STATUS[status].label)).toBeInTheDocument();
    const dot = container.querySelector<HTMLElement>('span[aria-hidden=true]');
    expect(dot).not.toBeNull();
    expect(dot?.style.backgroundColor).not.toBe('');
  });
});

describe('availableActions', () => {
  const all = {
    delete: true,
    install: true,
    reinstall: true,
    suspend: true,
    transfer: true
  };
  const none = {
    delete: false,
    install: false,
    reinstall: false,
    suspend: false,
    transfer: false
  };

  it.each([
    'running',
    'starting',
    'stopping',
    'crashed',
    'offline',
    'unknown',
    'install_failed',
    'not_installed'
  ] as const)('allows every action while %s', (status) => {
    expect(availableActions(status)).toEqual(all);
  });

  it('allows only the install toggle and suspend while installing', () => {
    expect(availableActions('installing')).toEqual({
      ...none,
      install: true,
      suspend: true
    });
  });

  it('allows install toggle, unsuspend and delete while suspended', () => {
    expect(availableActions('suspended')).toEqual({
      ...none,
      delete: true,
      install: true,
      suspend: true
    });
  });

  it('allows nothing while transferring', () => {
    expect(availableActions('transferring')).toEqual(none);
  });
});

describe('powerActions', () => {
  it.each([
    ['running', ['restart', 'stop', 'kill']],
    ['starting', ['stop', 'kill']],
    ['stopping', ['kill']],
    ['offline', ['start']],
    ['crashed', ['start']]
  ] as const)('allows %s: %j', (status, actions) => {
    expect(powerActions(status)).toEqual(actions);
  });

  it.each([
    'unknown',
    'installing',
    'install_failed',
    'not_installed',
    'suspended',
    'transferring'
  ] as const)('allows nothing while %s', (status) => {
    expect(powerActions(status)).toEqual([]);
  });

  it('never offers start alongside stop or kill', () => {
    for (const status of statuses) {
      const actions = powerActions(status);
      if (actions.includes('start')) {
        expect(actions).toEqual(['start']);
      }
    }
  });
});
