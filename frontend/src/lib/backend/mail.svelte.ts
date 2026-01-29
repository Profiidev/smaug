import { post, RequestError } from 'positron-components/backend';
import { fetchKey, getEncrypt } from './auth.svelte';

export interface ForgotPassword {
  email: string;
}

export const sendResetLink = async (data: ForgotPassword) => {
  return await post(`/api/mail/reset/send`, {
    body: data
  });
};

export interface ResetPassword {
  token: string;
  new_password: string;
}

export const sendResetPassword = async (data: ResetPassword) => {
  let encrypt = getEncrypt();
  if (!encrypt) {
    return RequestError.Other;
  }

  let encrypted_password = encrypt.encrypt(data.new_password);
  data.new_password = encrypted_password || '';

  let res = await post(`/api/mail/reset/confirm`, {
    body: data
  });
  if (res === RequestError.Unauthorized) {
    fetchKey();
  }

  return res;
};
