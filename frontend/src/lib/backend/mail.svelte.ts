import { post } from 'positron-components/backend';

export interface ForgotPassword {
  email: string;
}

export const sendResetLink = async (data: ForgotPassword) => {
  return await post(`/api/mail/reset/send`, {
    body: data
  });
};
