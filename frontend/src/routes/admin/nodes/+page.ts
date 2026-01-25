import { listNodes } from '$lib/backend/node.svelte';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, url }) => {
  return {
    error: url.searchParams.get('error'),
    nodes: await listNodes(fetch)
  };
};
