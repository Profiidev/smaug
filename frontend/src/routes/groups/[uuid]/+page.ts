import { RequestError } from '@profidev/pleiades/backend';
import type { PageLoad } from './$types';
import { redirect } from '@sveltejs/kit';
import { getGroupInfo, simpleUserList } from '$lib/backend/groups.svelte';

export const load: PageLoad = async ({ params, fetch }) => {
  const resPromise = getGroupInfo(params.uuid, fetch);
  const usersPromise = simpleUserList(fetch);

  const [res, users] = await Promise.all([resPromise, usersPromise]);

  if (typeof res !== 'object') {
    if (res === RequestError.NotFound) {
      redirect(307, '/groups?error=group_not_found');
    } else {
      redirect(307, '/groups?error=group_other');
    }
  }

  return {
    group: res,
    users,
    uuid: params.uuid
  };
};
