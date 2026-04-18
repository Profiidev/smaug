import { getGeneralSettings } from '$lib/backend/settings.svelte';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch }) => {
  const settings = await getGeneralSettings(fetch);
  return {
    settings
  };
};
