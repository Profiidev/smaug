import { listGroups } from '$lib/backend/groups.svelte';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, url }) => {
  let res = await listGroups(fetch);
  return {
    error: url.searchParams.get('error'),
    groups: res?.groups,
    admin_group: res?.admin_group
  };
};
