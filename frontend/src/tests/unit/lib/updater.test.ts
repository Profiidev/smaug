import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invalidate } from '$app/navigation';

let onMessage: (msg: unknown) => void = () => {};
vi.mock('@profidev/pleiades/backend', () => ({
  createWebsocket: () => ({
    connect: (cb: (msg: unknown) => void) => {
      onMessage = cb;
    },
    disconnect: vi.fn()
  })
}));

const { UpdateType, connectWebsocket } =
  await import('$lib/backend/updater.svelte');

describe('updater', () => {
  beforeEach(() => connectWebsocket('me'));

  it('invalidates the egg list and detail on Eggs updates', () => {
    onMessage({ type: UpdateType.Eggs, uuid: 'egg-1' });
    expect(vi.mocked(invalidate).mock.calls.map(([u]) => u)).toEqual([
      '/api/eggs',
      '/api/eggs/egg-1'
    ]);
  });
});
