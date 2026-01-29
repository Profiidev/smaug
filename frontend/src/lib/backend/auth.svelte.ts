import type JSEncrypt from 'jsencrypt';
import {
  ResponseType,
  RequestError,
  get,
  post
} from 'positron-components/backend';
import { browser } from '$app/environment';

let encrypt: false | undefined | JSEncrypt = $state(browser && undefined);

export const getEncrypt = () => {
  return encrypt;
};

export const fetchKey = async () => {
  if (encrypt === false) {
    return RequestError.Other;
  }

  let key = await get<{ key: string }>('/api/auth/password', {
    res_type: ResponseType.Json
  });

  if (typeof key !== 'object') {
    return key;
  }

  const JSEncrypt = (await import('jsencrypt')).JSEncrypt;

  encrypt = new JSEncrypt({ default_key_size: '4096' });
  encrypt.setPublicKey(key.key);
};
fetchKey();

export const passwordLogin = async (email: string, password: string) => {
  if (!encrypt) {
    return RequestError.Other;
  }

  let encrypted_password = encrypt.encrypt(password);
  let res = await post('/api/auth/password', {
    body: {
      email,
      password: encrypted_password
    }
  });

  if (res === RequestError.Unauthorized) {
    fetchKey();
  }
  return res;
};

export const logout = async () => {
  let res = await post('/api/auth/logout');

  return res;
};

export const testToken = async () => {
  let res = await get<boolean>('/api/auth/test_token', {
    res_type: ResponseType.Json
  });

  if (typeof res === 'boolean') {
    return res;
  }
};
