import { describe, expect, it } from 'vitest';
import { load as listLoad } from '$routes/users/+page';
import { load as createLoad } from '$routes/users/create/+page';
import { load as detailLoad } from '$routes/users/[uuid]/+page';
import { jsonFetch, runLoad } from '$test_helpers/load';

describe('users list load', () => {
  it('resolves users and exposes the error param', async () => {
    const result = runLoad(listLoad, {
      fetch: jsonFetch([{ uuid: 'u1' }]),
      url: new URL('http://x/users?error=e')
    });
    expect(result.error).toBe('e');
    await expect(result.users).resolves.toEqual([{ uuid: 'u1' }]);
  });
});

describe('users create load', () => {
  it('derives mailActive from the active flag', async () => {
    await expect(
      runLoad(createLoad, { fetch: jsonFetch({ active: true }) }).mailActive
    ).resolves.toBe(true);
    await expect(
      runLoad(createLoad, { fetch: jsonFetch(null) }).mailActive
    ).resolves.toBe(false);
  });
});

describe('user detail load', () => {
  it('passes the uuid and resolves the info + groups promises', async () => {
    const result = runLoad(detailLoad, {
      fetch: jsonFetch({ name: 'Bob' }),
      params: { uuid: 'u1' }
    });
    expect(result.uuid).toBe('u1');
    await expect(result.userInfoPromise).resolves.toMatchObject({
      data: { name: 'Bob' }
    });
    await expect(result.groupsPromise).resolves.toBeDefined();
  });
});
