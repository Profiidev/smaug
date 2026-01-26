import {
  get,
  post,
  RequestError,
  ResponseType
} from 'positron-components/backend';
import { fetch_key, getEncrypt } from './auth.svelte';

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
  let encrypt = getEncrypt();
  if (!encrypt) {
    return RequestError.Other;
  }

  let encrypted_password = encrypt.encrypt(payload.admin_password);
  payload.admin_password = encrypted_password || '';

  let res = await post('/api/setup', {
    body: payload
  });

  if (res === RequestError.Unauthorized) {
    fetch_key();
  }

  return res;
};
