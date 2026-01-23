import { listNodes } from '$lib/backend/node.svelte';
import type { PageLoad } from './$types';

export const load: PageLoad = async () => {
  return {
    nodes: listNodes()
  };
};
