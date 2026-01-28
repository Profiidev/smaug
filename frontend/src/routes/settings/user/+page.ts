import {
  getGeneralSettings,
  getUserSettings
} from '$lib/backend/settings.svelte';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch }) => {
  let fetchSettings = getUserSettings(fetch);
  let fetchGeneralSettings = getGeneralSettings(fetch);

  let [settings, generalSettings] = await Promise.all([
    fetchSettings,
    fetchGeneralSettings
  ]);

  return {
    settings,
    generalSettings
  };
};
