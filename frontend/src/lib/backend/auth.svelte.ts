import type JSEncrypt from 'jsencrypt';
import { ResponseType, RequestError, get } from 'positron-components/backend';
import { browser } from '$app/environment';

let encrypt: false | undefined | JSEncrypt = $state(browser && undefined);

export const getEncrypt = () => {
  return encrypt;
};

export const fetch_key = async () => {
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
fetch_key();
