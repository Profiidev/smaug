import { nodeInfo } from '$lib/backend/node.svelte';
import { RequestError } from 'positron-components/backend';
import type { PageLoad } from './$types';
import { redirect } from '@sveltejs/kit';

export const load: PageLoad = async ({ params, fetch }) => {
  let res = await nodeInfo(params.uuid, fetch);

  if (typeof res !== 'object') {
    if (res === RequestError.NotFound) {
      redirect(307, '/admin/nodes?error=node_not_found');
    } else {
      redirect(307, '/admin/nodes?error=node_other');
    }
  }

  return {
    uuid: params.uuid,
    node: res
  };
};
