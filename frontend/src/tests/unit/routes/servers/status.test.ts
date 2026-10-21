import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/svelte';
import StatusBadge from '$routes/servers/StatusBadge.svelte';
import {
  STATUS,
  STATUS_DESCRIPTIONS,
  type ServerStatus,
  availableActions
} from '$routes/servers/status';

afterEach(cleanup);

const statuses = Object.keys(STATUS) as ServerStatus[];

describe('STATUS map', () => {
  it('covers every server status', () => {
    expect(statuses.toSorted()).toEqual([
      'install_failed',
      'installing',
      'not_installed',
      'offline',
      'running',
      'starting',
      'stopping',
      'suspended',
      'transferring'
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

describe('STATUS_DESCRIPTIONS', () => {
  it('describes every status', () => {
    expect(Object.keys(STATUS_DESCRIPTIONS).toSorted()).toEqual(
      statuses.toSorted()
    );
    for (const text of Object.values(STATUS_DESCRIPTIONS)) {
      expect(text).not.toBe('');
    }
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
    'offline',
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
