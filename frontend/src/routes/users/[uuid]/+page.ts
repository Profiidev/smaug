import { RequestError } from 'positron-components/backend';
import type { PageLoad } from './$types';
import { redirect } from '@sveltejs/kit';
import { getListUserInfo, simpleGroupList } from '$lib/backend/user.svelte';

export const load: PageLoad = async ({ params, fetch }) => {
  let resPromise = getListUserInfo(params.uuid, fetch);
  let groupsPromise = simpleGroupList(fetch);

  let [res, groups] = await Promise.all([resPromise, groupsPromise]);

  if (typeof res !== 'object') {
    if (res === RequestError.NotFound) {
      redirect(307, '/users?error=user_not_found');
    } else {
      redirect(307, '/users?error=user_other');
    }
  }

  return {
    uuid: params.uuid,
    userInfo: res,
    groups
  };
};
