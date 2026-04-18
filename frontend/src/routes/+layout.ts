import { getSetupStatus } from '$lib/backend/setup.svelte';
import { redirect } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';
import { type UserInfo, getUserInfo } from '$lib/backend/user.svelte';
import { noSidebarPaths } from '$lib/components/navigation/sidebar/items.svelte';
import { RequestError } from '@profidev/pleiades/backend';

export const load: LayoutLoad = async ({ fetch, url }) => {
  const status = await getSetupStatus(fetch);

  if (!status?.is_setup && url.pathname !== '/setup') {
    redirect(302, '/setup');
  }

  let user: UserInfo | RequestError | undefined = await getUserInfo(fetch);

  if (
    typeof user === 'string' &&
    user !== RequestError.Unauthorized &&
    !noSidebarPaths.includes(url.pathname)
  ) {
    redirect(302, '/login');
  }

  if (typeof user === 'string') {
    user = undefined;
  }

  return {
    user
  };
};
