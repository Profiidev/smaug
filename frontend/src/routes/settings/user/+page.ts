import {
  getGeneralSettings,
  getUserSettings
} from '$lib/backend/settings.svelte';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch }) => {
  const fetchSettings = getUserSettings(fetch);
  const fetchGeneralSettings = getGeneralSettings(fetch);

  const [settings, generalSettings] = await Promise.all([
    fetchSettings,
    fetchGeneralSettings
  ]);

  return {
    generalSettings,
    settings
  };
};
