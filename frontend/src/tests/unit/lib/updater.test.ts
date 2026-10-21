import { describe, expect, it, vi } from 'vitest';
import { invalidate } from '$app/navigation';

let onMessage: (msg: unknown) => void = () => {};
const disconnect = vi.fn();
vi.mock('@profidev/pleiades/backend', () => ({
  createWebsocket: () => ({
    connect: (cb: (msg: unknown) => void) => {
      onMessage = cb;
    },
    disconnect: () => disconnect()
  })
}));

const { UpdateType, connectWebsocket, disconnectWebsocket } =
  await import('$lib/backend/updater.svelte');

/** Sends `msg` to the handler registered for `user`. */
const send = (msg: unknown, user = 'me') => {
  connectWebsocket(user);
  onMessage(msg);
};

/** The string urls passed to invalidate (ignores predicate calls). */
const invalidatedUrls = () =>
  vi
    .mocked(invalidate)
    .mock.calls.map(([u]) => u)
    .filter((u): u is string => typeof u === 'string');

describe('UpdateType enum', () => {
  it('exposes every update kind as a matching string', () => {
    expect(Object.entries(UpdateType)).toEqual(
      Object.values(UpdateType).map((v) => [v, v])
    );
    expect(Object.values(UpdateType).toSorted()).toEqual([
      'Eggs',
      'Group',
      'Nodes',
      'Servers',
      'Settings',
      'User',
      'UserPermissions'
    ]);
  });
});

describe('disconnectWebsocket', () => {
  it('delegates to the pleiades socket', () => {
    disconnectWebsocket();
    expect(disconnect).toHaveBeenCalledOnce();
  });
});

describe('updater message handling', () => {
  it('invalidates the settings endpoints via a path predicate', () => {
    send({ type: UpdateType.Settings });
    expect(invalidate).toHaveBeenCalledOnce();
    const [[predicate]] = vi.mocked(invalidate).mock.calls;
    expect(typeof predicate).toBe('function');
    const matches = predicate as (u: URL) => boolean;
    expect(matches(new URL('http://x/api/settings/mail'))).toBe(true);
    expect(matches(new URL('http://x/api/other'))).toBe(false);
  });

  it('invalidates the user endpoints and the own info for the current user', () => {
    send({ type: UpdateType.User, uuid: 'me' }, 'me');
    expect(invalidatedUrls()).toEqual([
      '/api/user/management',
      '/api/user/management/me',
      '/api/group/users',
      '/api/user/info'
    ]);
  });

  it('does not invalidate the own info for another user', () => {
    send({ type: UpdateType.User, uuid: 'other' }, 'me');
    expect(invalidatedUrls()).toEqual([
      '/api/user/management',
      '/api/user/management/other',
      '/api/group/users'
    ]);
  });

  it('invalidates only the own info on UserPermissions updates', () => {
    send({ type: UpdateType.UserPermissions });
    expect(invalidatedUrls()).toEqual(['/api/user/info']);
  });

  it('invalidates the group endpoints on Group updates', () => {
    send({ type: UpdateType.Group, uuid: 'g1' });
    expect(invalidatedUrls()).toEqual([
      '/api/group',
      '/api/group/g1',
      '/api/user/management/groups'
    ]);
  });

  it('invalidates the node list and detail on Nodes updates', () => {
    send({ type: UpdateType.Nodes, uuid: 'n1' });
    expect(invalidatedUrls()).toEqual(['/api/nodes', '/api/nodes/n1']);
  });

  it('invalidates the egg list and detail on Eggs updates', () => {
    send({ type: UpdateType.Eggs, uuid: 'egg-1' });
    expect(invalidatedUrls()).toEqual(['/api/eggs', '/api/eggs/egg-1']);
  });

  it('invalidates only the server list and that server on Servers updates', () => {
    send({ type: UpdateType.Servers, uuid: 's1' });
    // Other servers' edit pages (/api/servers/s2) and options stay untouched.
    expect(invalidate).toHaveBeenCalledTimes(2);
    expect(invalidatedUrls()).toEqual(['/api/servers', '/api/servers/s1']);
  });

  it('ignores unknown message types', () => {
    send({ type: 'Nonsense' });
    expect(invalidate).not.toHaveBeenCalled();
  });

  it('swallows invalidate rejections', async () => {
    vi.mocked(invalidate)
      .mockRejectedValueOnce(new Error('boom'))
      .mockRejectedValueOnce(new Error('boom'));
    send({ type: UpdateType.Nodes, uuid: 'n1' });
    // A rejected invalidate must not surface as an unhandled rejection.
    await Promise.resolve();
    expect(invalidate).toHaveBeenCalledTimes(2);
  });
});
