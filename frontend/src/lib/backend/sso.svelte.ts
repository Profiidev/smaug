import { ResponseType, get } from '@profidev/pleiades/backend';

export enum SSOType {
  None = 'None',
  Oidc = 'Oidc'
}

export interface AuthConfig {
  sso_type: SSOType;
  instant_redirect: boolean;
  mail_enabled: boolean;
}

export const getAuthConfig = async (
  fetch: typeof window.fetch = window.fetch
) => {
  const res = await get<AuthConfig>('/api/auth/config', {
    fetch,
    res_type: ResponseType.Json
  });
  if (typeof res === 'object') {
    return res;
  }
  return undefined;
};

export const getOidcUrl = async () => {
  const res = await get<{ url: string }>('/api/auth/oidc/url', {
    res_type: ResponseType.Json
  });

  if (typeof res === 'object') {
    return res.url;
  }
  return undefined;
};
