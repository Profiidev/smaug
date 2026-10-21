import { nodeInfo } from '$lib/client';
import type { PageLoad } from './$types';

export const load = (({ fetch, params, url }) => ({
  nodeRes: nodeInfo({ fetch, path: { uuid: params.uuid } }),
  tab: url.searchParams.get('tab'),
  uuid: params.uuid
})) satisfies PageLoad;
