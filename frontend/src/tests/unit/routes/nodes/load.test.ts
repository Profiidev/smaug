import { describe, expect, it } from 'vitest';
import { load as listLoad } from '$routes/nodes/+page';
import { load as rootLoad } from '$routes/nodes/[uuid]/+page';
import { load as detailLoad } from '$routes/nodes/[uuid]/[tab=nodetab]/+page';
import { match } from '../../../../params/nodetab';
import { catchRedirect, jsonFetch, runLoad } from '$test_helpers/load';
import type { NodeInfo } from '$lib/client';

const node: NodeInfo = {
  address: 'wings.example.com',
  connected: false,
  id: 'n1',
  name: 'Alpha',
  port: 443,
  secure: true
};

const url = (q = '') => new URL(`http://localhost/nodes${q}`);
const requested = (f: ReturnType<typeof jsonFetch>) => {
  const req = f.mock.calls[0][0] as Request;
  return { method: req.method, path: new URL(req.url).pathname };
};

describe('nodes list load', () => {
  it('resolves the nodes from listNodes without blocking', async () => {
    const fetch = jsonFetch([node]);
    const result = runLoad(listLoad, { fetch, url: url() });
    expect(result.nodes).toBeInstanceOf(Promise);
    await expect(result.nodes).resolves.toEqual([node]);
    expect(requested(fetch)).toEqual({ method: 'GET', path: '/api/nodes' });
  });

  it('resolves an empty list', async () => {
    const result = runLoad(listLoad, { fetch: jsonFetch([]), url: url() });
    await expect(result.nodes).resolves.toEqual([]);
  });

  it.each([401, 403, 500])('resolves undefined on %i', async (status) => {
    const result = runLoad(listLoad, {
      fetch: jsonFetch(null, status),
      url: url()
    });
    await expect(result.nodes).resolves.toBeUndefined();
  });

  it.each([
    ['', null],
    ['?error=node_not_found', 'node_not_found'],
    ['?error=node_other', 'node_other']
  ])('passes the error query %j through', (q, error) => {
    const result = runLoad(listLoad, { fetch: jsonFetch([]), url: url(q) });
    expect(result.error).toBe(error);
  });
});

describe('node [uuid] page load', () => {
  const detailUrl = (q = '') => new URL(`http://localhost/nodes/n1${q}`);

  it('requests the node by uuid and keeps the uuid', async () => {
    const fetch = jsonFetch(node);
    const result = runLoad(detailLoad, {
      fetch,
      params: { uuid: 'n1' },
      url: detailUrl()
    });
    expect(result.uuid).toBe('n1');
    expect(result.nodeRes).toBeInstanceOf(Promise);
    expect((await result.nodeRes).data).toEqual(node);
    expect(requested(fetch)).toEqual({ method: 'GET', path: '/api/nodes/n1' });
  });

  it('does not read the tab param (switching tabs must not reload)', () => {
    const params = new Proxy(
      { uuid: 'n1' },
      {
        get: (t, k) => {
          if (k === 'tab') {
            throw new Error('load read params.tab');
          }
          return t[k as 'uuid'];
        }
      }
    );
    const result = runLoad(detailLoad, {
      fetch: jsonFetch(node),
      params,
      url: detailUrl('/general')
    });
    expect(Object.keys(result).toSorted()).toEqual(['nodeRes', 'uuid']);
  });

  it.each([404, 500])('exposes the %i status for the redirect', async (s) => {
    const result = runLoad(detailLoad, {
      fetch: jsonFetch(null, s),
      params: { uuid: 'nope' },
      url: detailUrl()
    });
    const res = await result.nodeRes;
    expect(res.data).toBeUndefined();
    expect(res.response?.status).toBe(s);
  });
});

describe('nodetab param matcher', () => {
  it.each(['setup', 'general', 'resources'])('accepts %s', (tab) => {
    expect(match(tab)).toBe(true);
  });

  it.each(['settings', 'bogus', '', 'Setup', 'general/'])(
    'rejects %j',
    (tab) => {
      expect(match(tab)).toBe(false);
    }
  );
});

describe('node [uuid] without a tab', () => {
  it('redirects to the setup tab', async () => {
    const redirect = await catchRedirect(() =>
      (rootLoad as (e: unknown) => unknown)({ params: { uuid: 'n1' } })
    );
    expect(redirect.status).toBe(307);
    expect(redirect.location).toBe('/nodes/n1/setup');
  });
});
