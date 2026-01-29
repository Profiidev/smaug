import { getMailSettings } from '$lib/backend/settings.svelte';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch }) => {
  let settings = await getMailSettings(fetch);
  return {
    settings
  };
};
