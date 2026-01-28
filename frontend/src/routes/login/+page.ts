import { getOidcUrl, getAuthConfig, SSOType } from '$lib/backend/sso.svelte';
import { redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, url }) => {
  let error = url.searchParams.get('error') || null;
  if (error) {
    return { error };
  }
  let skip = url.searchParams.get('skip') === 'true';

  let config = await getAuthConfig(fetch);
  if (config?.sso_type !== SSOType.None) {
    let url = await getOidcUrl();
    if (url && config?.instant_redirect && !skip) {
      redirect(302, url);
    }
    return { oidc_url: url, config, skip };
  }
  return { config, skip };
};
