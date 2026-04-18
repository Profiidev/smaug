import { RequestError, post } from '@profidev/pleiades/backend';
import { fetchKey, getEncrypt } from './auth.svelte';

export interface ForgotPassword {
  email: string;
}

export const sendResetLink = async (data: ForgotPassword) =>
  await post(`/api/mail/reset/send`, {
    body: data
  });

export interface ResetPassword {
  token: string;
  new_password: string;
}

export const sendResetPassword = async (data: ResetPassword) => {
  const encrypt = getEncrypt();
  if (!encrypt) {
    return RequestError.Other;
  }

  const encrypted_password = encrypt.encrypt(data.new_password);
  data.new_password = encrypted_password || '';

  const res = await post(`/api/mail/reset/confirm`, {
    body: data
  });
  if (res === RequestError.Unauthorized) {
    const _ = fetchKey();
  }

  return res;
};

export const sendTestEmail = async () => await post(`/api/mail/test`);
