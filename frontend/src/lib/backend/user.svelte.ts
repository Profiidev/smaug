import type { Permission } from '$lib/permissions.svelte';
import { get, post, ResponseType } from 'positron-components/backend';

export interface UserInfo {
  uuid: string;
  name: string;
  email: string;
  permissions: Permission[];
  avatar?: string;
}

export const getUserInfo = async (
  fetch: typeof window.fetch = window.fetch
) => {
  return await get<UserInfo>('/api/user/info', {
    res_type: ResponseType.Json,
    fetch
  });
};

export interface AccountUpdate {
  username: string;
}

export const updateAccount = async (data: AccountUpdate) => {
  return await post('/api/user/account/update', {
    body: data
  });
};

export interface AvatarUpdate {
  avatar: string;
}

export const updateAvatar = async (data: AvatarUpdate) => {
  return await post('/api/user/account/avatar', {
    body: data
  });
};
