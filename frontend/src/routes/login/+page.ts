import { SSOType, getAuthConfig, getOidcUrl } from '$lib/backend/sso.svelte';
import { redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, url }) => {
  const error = url.searchParams.get('error') || undefined;
  if (error) {
    return { error };
  }
  const skip = url.searchParams.get('skip') === 'true';

  const config = await getAuthConfig(fetch);
  if (config?.sso_type !== SSOType.None) {
    const oidcUrl = await getOidcUrl();
    if (oidcUrl && config?.instant_redirect && !skip) {
      redirect(302, oidcUrl);
    }
    return { config, oidc_url: oidcUrl, skip };
  }
  return { config, skip };
};
