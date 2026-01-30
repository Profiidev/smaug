import { RequestError } from 'positron-components/backend';
import type { PageLoad } from './$types';
import { redirect } from '@sveltejs/kit';
import { getGroupInfo } from '$lib/backend/groups.svelte';

export const load: PageLoad = async ({ params, fetch }) => {
  let res = await getGroupInfo(params.uuid, fetch);

  if (typeof res !== 'object') {
    if (res === RequestError.NotFound) {
      redirect(307, '/groups?error=group_not_found');
    } else {
      redirect(307, '/groups?error=group_other');
    }
  }

  return {
    uuid: params.uuid,
    group: res
  };
};
