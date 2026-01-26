import { getSetupStatus } from '$lib/backend/setup.svelte';
import { redirect } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ fetch, url }) => {
  let status = await getSetupStatus(fetch);

  if (!status?.is_setup && url.pathname !== '/setup') {
    redirect(302, '/setup');
  }
};
