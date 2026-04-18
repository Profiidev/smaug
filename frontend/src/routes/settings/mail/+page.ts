import { getMailSettings } from '$lib/backend/settings.svelte';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch }) => {
  const settings = await getMailSettings(fetch);
  return {
    settings
  };
};
