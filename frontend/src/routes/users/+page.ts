import { listUsers } from '$lib/backend/user.svelte';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, url }) => {
  let users = await listUsers(fetch);
  return {
    error: url.searchParams.get('error'),
    users
  };
};
