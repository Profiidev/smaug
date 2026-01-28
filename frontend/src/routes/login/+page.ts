import { getOidcUrl, getSSOConfig, SSOType } from '$lib/backend/sso.svelte';
import { redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, url }) => {
  let error = url.searchParams.get('error') || null;
  if (error) {
    return { error };
  }
  let skip = url.searchParams.get('skip') === 'true';
  if (skip) {
    return;
  }

  let config = await getSSOConfig(fetch);
  if (config?.sso_type !== SSOType.None) {
    let url = await getOidcUrl();
    if (url && config?.instant_redirect) {
      redirect(302, url);
    }
    return { oidc_url: url, sso_config: config };
  }
  return { sso_config: config };
};
