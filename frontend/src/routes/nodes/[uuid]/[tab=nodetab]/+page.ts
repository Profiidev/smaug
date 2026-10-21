import { nodeInfo } from '$lib/client';
import type { PageLoad } from './$types';

// Only params.uuid: switching tabs must not rerun this.
export const load = (({ fetch, params }) => ({
  nodeRes: nodeInfo({ fetch, path: { uuid: params.uuid } }),
  uuid: params.uuid
})) satisfies PageLoad;
