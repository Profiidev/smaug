import { RequestError } from 'positron-components/backend';
import type { PageLoad } from './$types';
import { redirect } from '@sveltejs/kit';
import { getGroupInfo, simpleUserList } from '$lib/backend/groups.svelte';

export const load: PageLoad = async ({ params, fetch }) => {
  let resPromise = getGroupInfo(params.uuid, fetch);
  let usersPromise = simpleUserList(fetch);

  let [res, users] = await Promise.all([resPromise, usersPromise]);

  if (typeof res !== 'object') {
    if (res === RequestError.NotFound) {
      redirect(307, '/groups?error=group_not_found');
    } else {
      redirect(307, '/groups?error=group_other');
    }
  }

  return {
    uuid: params.uuid,
    group: res,
    users
  };
};
