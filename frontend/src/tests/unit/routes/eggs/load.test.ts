import { describe, expect, it } from 'vitest';
import { load as listLoad } from '$routes/eggs/+page';
import { load as detailLoad } from '$routes/eggs/[uuid]/+page';
import { jsonFetch, runLoad } from '$test_helpers/load';
import { eggInfo, eggList } from '$mocks/e2e/data';

const url = (q = '') => new URL(`http://localhost/eggs${q}`);
const requested = (f: ReturnType<typeof jsonFetch>) => {
  const req = f.mock.calls[0][0] as Request;
  return { method: req.method, path: new URL(req.url).pathname };
};

describe('eggs list load', () => {
  it('resolves the eggs from listEggs', async () => {
    const fetch = jsonFetch(eggList.default);
    const result = runLoad(listLoad, { fetch, url: url() });
    await expect(result.eggs).resolves.toEqual(eggList.default.eggs);
    expect(requested(fetch)).toEqual({ method: 'GET', path: '/api/eggs' });
  });

  it('resolves an empty list', async () => {
    const result = runLoad(listLoad, {
      fetch: jsonFetch({ eggs: [] }),
      url: url()
    });
    await expect(result.eggs).resolves.toEqual([]);
  });

  it.each([401, 403, 500])('resolves undefined on %i', async (status) => {
    const result = runLoad(listLoad, {
      fetch: jsonFetch(null, status),
      url: url()
    });
    await expect(result.eggs).resolves.toBeUndefined();
  });

  it.each([
    ['', null],
    ['?error=not_found', 'not_found'],
    ['?error=other', 'other']
  ])('passes the error query %j through', (q, error) => {
    const result = runLoad(listLoad, {
      fetch: jsonFetch(eggList.default),
      url: url(q)
    });
    expect(result.error).toBe(error);
  });

  it('does not block on the request (returns a Promise)', () => {
    const result = runLoad(listLoad, {
      fetch: jsonFetch(eggList.default),
      url: url()
    });
    expect(result.eggs).toBeInstanceOf(Promise);
  });
});

describe('egg detail load', () => {
  it('requests the egg by uuid and keeps the uuid', async () => {
    const fetch = jsonFetch(eggInfo);
    const result = runLoad(detailLoad, {
      fetch,
      params: { uuid: 'egg-paper' }
    });
    expect(result.uuid).toBe('egg-paper');
    expect(result.eggRes).toBeInstanceOf(Promise);
    const res = await result.eggRes;
    expect(res.data).toEqual(eggInfo);
    expect(requested(fetch)).toEqual({
      method: 'GET',
      path: '/api/eggs/egg-paper'
    });
  });

  it.each([404, 500])(
    'exposes the %i status for the redirect',
    async (status) => {
      const result = runLoad(detailLoad, {
        fetch: jsonFetch(null, status),
        params: { uuid: 'nope' }
      });
      const res = await result.eggRes;
      expect(res.data).toBeUndefined();
      expect(res.response?.status).toBe(status);
    }
  );
});
