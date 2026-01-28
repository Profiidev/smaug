import { getSetupStatus } from '$lib/backend/setup.svelte';
import { redirect } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';
import { getUserInfo } from '$lib/backend/user.svelte';
import { noSidebarPaths } from '$lib/components/navigation/sidebar/items.svelte';

export const load: LayoutLoad = async ({ fetch, url }) => {
  let status = await getSetupStatus(fetch);

  if (!status?.is_setup && url.pathname !== '/setup') {
    redirect(302, '/setup');
  }

  let user = await getUserInfo(fetch);

  if (!user && !noSidebarPaths.includes(url.pathname)) {
    redirect(302, '/login');
  }

  return {
    user
  };
};
