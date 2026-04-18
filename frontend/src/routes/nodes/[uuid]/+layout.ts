import { nodeInfo } from '$lib/backend/node.svelte';
import { RequestError } from '@profidev/pleiades/backend';
import type { LayoutLoad } from './$types';
import { redirect } from '@sveltejs/kit';

export const load: LayoutLoad = async ({ params, fetch }) => {
  const res = await nodeInfo(params.uuid, fetch);

  if (typeof res !== 'object') {
    if (res === RequestError.NotFound) {
      redirect(307, '/nodes?error=node_not_found');
    } else {
      redirect(307, '/nodes?error=node_other');
    }
  }

  return {
    node: res,
    uuid: params.uuid
  };
};
