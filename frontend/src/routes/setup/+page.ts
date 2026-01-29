import { getSetupStatus } from '$lib/backend/setup.svelte';
import { redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch }) => {
  let status = await getSetupStatus(fetch);

  if (status?.is_setup) {
    redirect(302, '/');
  }

  return {
    db_backend: status?.db_backend ?? 'unknown'
  };
};
