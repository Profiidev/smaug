import type JSEncrypt from 'jsencrypt';
import {
  RequestError,
  ResponseType,
  get,
  post
} from '@profidev/pleiades/backend';
import { browser } from '$app/environment';

let encrypt: false | undefined | JSEncrypt = $state(browser && undefined);

export const getEncrypt = () => encrypt;

export const fetchKey = async () => {
  if (encrypt === false) {
    return RequestError.Other;
  }

  const key = await get<{ key: string }>('/api/auth/password', {
    res_type: ResponseType.Json
  });

  if (typeof key !== 'object') {
    return key;
  }

  const { JSEncrypt } = await import('jsencrypt');

  encrypt = new JSEncrypt({ default_key_size: '4096' });
  encrypt.setPublicKey(key.key);
  return undefined;
};
const _ = fetchKey();

export const passwordLogin = async (email: string, password: string) => {
  if (!encrypt) {
    return RequestError.Other;
  }

  const encrypted_password = encrypt.encrypt(password);
  const res = await post('/api/auth/password', {
    body: {
      email,
      password: encrypted_password
    }
  });

  if (res === RequestError.Unauthorized) {
    const _f = fetchKey();
  }
  return res;
};

export const logout = async () => {
  const res = await post('/api/auth/logout');

  return res;
};

export const testToken = async () => {
  const res = await get<boolean>('/api/auth/test_token', {
    res_type: ResponseType.Json
  });

  if (typeof res === 'boolean') {
    return res;
  }
  return undefined;
};
