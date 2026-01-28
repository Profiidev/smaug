import { get, ResponseType } from 'positron-components/backend';

export enum SSOType {
  None = 'None',
  Oidc = 'Oidc'
}

export interface SSOConfig {
  sso_type: SSOType;
  instant_redirect: boolean;
}

export const getSSOConfig = async (
  fetch: typeof window.fetch = window.fetch
) => {
  let res = await get<SSOConfig>('/api/auth/sso', {
    res_type: ResponseType.Json,
    fetch
  });
  if (typeof res === 'object') {
    return res;
  }
};

export const getOidcUrl = async () => {
  let res = await get<{ url: string }>('/api/auth/oidc/url', {
    res_type: ResponseType.Json
  });

  if (typeof res === 'object') {
    return res.url;
  }
};
