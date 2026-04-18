import { getMailStatus } from '$lib/backend/user.svelte';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch }) => {
  const active = await getMailStatus(fetch);
  return {
    mailActive: active?.active ?? false
  };
};
