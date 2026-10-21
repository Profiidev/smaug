import { listServers } from '$lib/client';
import type { PageLoad } from './$types';

export const load = (({ fetch, url }) => ({
  error: url.searchParams.get('error'),
  servers: listServers({ fetch }).then((r) => r.data?.servers)
})) satisfies PageLoad;
