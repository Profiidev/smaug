import { describe, expect, it, vi } from 'vitest';
import { load as listLoad } from '$routes/servers/+page';
import { load as createLoad } from '$routes/servers/create/+page';
import { load as editLoad } from '$routes/servers/[uuid]/+page';
import { jsonFetch, runLoad } from '$test_helpers/load';
import { options, serverInfo, servers } from './fixtures';

const url = (q = '') => new URL(`http://localhost/servers${q}`);
const requested = (f: ReturnType<typeof jsonFetch>) => {
  const req = f.mock.calls[0][0] as Request;
  return { method: req.method, path: new URL(req.url).pathname };
};

describe('servers list load', () => {
  it('resolves the servers from listServers', async () => {
    const fetch = jsonFetch({ servers });
    const result = runLoad(listLoad, { fetch, url: url() });
    expect(result.servers).toBeInstanceOf(Promise);
    await expect(result.servers).resolves.toEqual(servers);
    expect(requested(fetch)).toEqual({ method: 'GET', path: '/api/servers' });
  });

  it('resolves an empty list', async () => {
    const result = runLoad(listLoad, {
      fetch: jsonFetch({ servers: [] }),
      url: url()
    });
    await expect(result.servers).resolves.toEqual([]);
  });

  it.each([401, 403, 500])('resolves undefined on %i', async (status) => {
    const result = runLoad(listLoad, {
      fetch: jsonFetch(null, status),
      url: url()
    });
    await expect(result.servers).resolves.toBeUndefined();
  });

  it.each([
    ['', null],
    ['?error=not_found', 'not_found'],
    ['?error=options', 'options']
  ])('passes the error query %j through', (q, error) => {
    const result = runLoad(listLoad, {
      fetch: jsonFetch({ servers }),
      url: url(q)
    });
    expect(result.error).toBe(error);
  });
});

describe('server create load', () => {
  it('resolves the server options without blocking', async () => {
    const fetch = jsonFetch(options);
    const result = runLoad(createLoad, { fetch });
    expect(result.options).toBeInstanceOf(Promise);
    await expect(result.options).resolves.toEqual(options);
    expect(requested(fetch)).toEqual({
      method: 'GET',
      path: '/api/servers/options'
    });
  });

  it.each([403, 500])('resolves undefined on %i', async (status) => {
    const result = runLoad(createLoad, { fetch: jsonFetch(null, status) });
    await expect(result.options).resolves.toBeUndefined();
  });
});

describe('server edit load', () => {
  /** Answers by path: the server info (or `status`) and the options. */
  const routes = (info: unknown, status = 200, optionsStatus = 200) =>
    vi.fn(async (input: RequestInfo | URL) => {
      const { pathname } = new URL((input as Request).url);
      const [body, code] =
        pathname === '/api/servers/options'
          ? [options, optionsStatus]
          : [info, status];
      return new Response(code === 200 ? JSON.stringify(body) : null, {
        headers: { 'Content-Type': 'application/json' },
        status: code
      });
    });
  const paths = (f: ReturnType<typeof routes>) =>
    f.mock.calls.map(([r]) => new URL((r as Request).url).pathname);
  const params = { uuid: 'srv-1' };

  it('returns promises and the uuid without blocking', () => {
    const result = runLoad(editLoad, { fetch: routes(serverInfo), params });
    expect(result.serverRes).toBeInstanceOf(Promise);
    expect(result.options).toBeInstanceOf(Promise);
    expect(result.uuid).toBe('srv-1');
  });

  it('loads the server and, for admins, the options', async () => {
    const fetch = routes(serverInfo);
    const result = runLoad(editLoad, { fetch, params });
    const res = await result.serverRes;
    expect(res.data?.name).toBe('survival');
    // The JSON string becomes a Date through the SDK's response transformer.
    expect(res.data?.created_at).toEqual(serverInfo.created_at);
    expect(res.data?.created_at).toBeInstanceOf(Date);
    await expect(result.options).resolves.toEqual(options);
    expect(paths(fetch)).toEqual([
      '/api/servers/srv-1',
      '/api/servers/options'
    ]);
  });

  it.each(['write', 'read'])(
    'skips the options for %s access',
    async (access) => {
      const fetch = routes({ ...serverInfo, access, grants: null });
      const result = runLoad(editLoad, { fetch, params });
      await expect(result.options).resolves.toBeUndefined();
      expect(paths(fetch)).toEqual(['/api/servers/srv-1']);
    }
  );

  it.each([404, 500])(
    'passes a %i through and skips the options',
    async (status) => {
      const fetch = routes(null, status);
      const result = runLoad(editLoad, { fetch, params });
      const res = await result.serverRes;
      expect(res.data).toBeUndefined();
      expect(res.response?.status).toBe(status);
      await expect(result.options).resolves.toBeUndefined();
      expect(paths(fetch)).toEqual(['/api/servers/srv-1']);
    }
  );

  it('resolves undefined options when they fail', async () => {
    const result = runLoad(editLoad, {
      fetch: routes(serverInfo, 200, 403),
      params
    });
    await expect(result.options).resolves.toBeUndefined();
  });
});
