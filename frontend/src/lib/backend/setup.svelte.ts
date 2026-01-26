import { get, post, ResponseType } from 'positron-components/backend';

export interface SetupPayload {
  admin_username: string;
  admin_password: string;
  admin_email: string;
}

export interface SetupStatus {
  is_setup: boolean;
  db_backend: 'PostgreSQL' | 'MySQL' | 'SQLite';
}

export const getSetupStatus = async (
  fetch: typeof window.fetch = window.fetch
) => {
  let ret = await get<SetupStatus>('/api/setup', {
    res_type: ResponseType.Json,
    fetch
  });

  if (typeof ret === 'object') {
    return ret;
  }
};

export const performSetup = async (payload: SetupPayload) => {
  return await post('/api/setup', {
    body: payload
  });
};
