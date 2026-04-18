import {
  RequestError,
  ResponseType,
  get,
  post
} from '@profidev/pleiades/backend';
import { fetchKey, getEncrypt } from './auth.svelte';

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
  const ret = await get<SetupStatus>('/api/setup', {
    fetch,
    res_type: ResponseType.Json
  });

  if (typeof ret === 'object') {
    return ret;
  }
  return undefined;
};

export const performSetup = async (payload: SetupPayload) => {
  const encrypt = getEncrypt();
  if (!encrypt) {
    return RequestError.Other;
  }

  const encrypted_password = encrypt.encrypt(payload.admin_password);
  payload.admin_password = encrypted_password || '';

  const res = await post('/api/setup', {
    body: payload
  });

  if (res === RequestError.Unauthorized) {
    const _ = fetchKey();
  }

  return res;
};
