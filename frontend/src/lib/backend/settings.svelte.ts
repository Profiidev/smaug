import { ResponseType, get, post } from '@profidev/pleiades/backend';

const getSettings = async <T>(name: string, fetch: typeof window.fetch) => {
  const res = await get<T>(`/api/settings/${name}`, {
    fetch,
    res_type: ResponseType.Json
  });
  if (typeof res === 'object') {
    return res;
  }
  return undefined;
};

// oxlint-disable-next-line no-unnecessary-type-parameters
const saveSettings = async <T>(name: string, settings: T) =>
  await post(`/api/settings/${name}`, {
    body: settings
  });

export interface GeneralSettings {
  site_url: string;
}
export const getGeneralSettings = async (fetch: typeof window.fetch) =>
  await getSettings<GeneralSettings>('general', fetch);
export const saveGeneralSettings = async (settings: GeneralSettings) =>
  await saveSettings<GeneralSettings>('general', settings);

export interface UserSettings {
  sso_instant_redirect: boolean;
  sso_create_user: boolean;
  oidc?: OidcSettings;
}
export interface OidcSettings {
  issuer: string;
  client_id: string;
  client_secret: string;
  scopes: string[];
}
export const getUserSettings = async (fetch: typeof window.fetch) =>
  await getSettings<UserSettings>('user', fetch);
export const saveUserSettings = async (settings: UserSettings) =>
  await saveSettings<UserSettings>('user', settings);

export interface MailSettings {
  smtp?: SmtpSettings;
}
export interface SmtpSettings {
  server: string;
  port: number;
  username: string;
  password: string;
  use_tls: boolean;
  from_address: string;
  from_name: string;
}
export const getMailSettings = async (fetch: typeof window.fetch) =>
  await getSettings<MailSettings>('mail', fetch);
export const saveMailSettings = async (settings: MailSettings) =>
  await saveSettings<MailSettings>('mail', settings);
