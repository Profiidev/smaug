import { RequestError } from '@profidev/pleiades/backend';
import type { PageLoad } from './$types';
import { redirect } from '@sveltejs/kit';
import {
  getListUserInfo,
  getMailStatus,
  simpleGroupList
} from '$lib/backend/user.svelte';

export const load: PageLoad = async ({ params, fetch }) => {
  const resPromise = getListUserInfo(params.uuid, fetch);
  const groupsPromise = simpleGroupList(fetch);
  const mailPromise = getMailStatus(fetch);

  const [res, groups, mail] = await Promise.all([
    resPromise,
    groupsPromise,
    mailPromise
  ]);

  if (typeof res !== 'object') {
    if (res === RequestError.NotFound) {
      redirect(307, '/users?error=user_not_found');
    } else {
      redirect(307, '/users?error=user_other');
    }
  }

  return {
    groups,
    mailActive: mail?.active ?? false,
    userInfo: res,
    uuid: params.uuid
  };
};
