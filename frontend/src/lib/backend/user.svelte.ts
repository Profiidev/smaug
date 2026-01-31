import type { Permission } from '$lib/permissions.svelte';
import {
  delete_,
  get,
  post,
  RequestError,
  ResponseType
} from 'positron-components/backend';
import { fetchKey, getEncrypt } from './auth.svelte';

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

export interface UpdatePassword {
  old_password: string;
  new_password: string;
}

export const updatePassword = async (payload: UpdatePassword) => {
  let encrypt = getEncrypt();
  if (!encrypt) {
    return RequestError.Other;
  }

  let encrypted_password = encrypt.encrypt(payload.old_password);
  payload.old_password = encrypted_password || '';
  encrypted_password = encrypt.encrypt(payload.new_password);
  payload.new_password = encrypted_password || '';

  let res = await post('/api/user/account/password', {
    body: payload
  });

  if (res === RequestError.Unauthorized) {
    fetchKey();
  }

  return res;
};

export interface UserListInfo {
  uuid: string;
  name: string;
  email: string;
  avatar?: string;
  groups: SimpleGroupInfo[];
}

export interface SimpleGroupInfo {
  uuid: string;
  name: string;
}

export const listUsers = async (fetch: typeof window.fetch = window.fetch) => {
  let ret = await get<UserListInfo[]>('/api/user/management', {
    res_type: ResponseType.Json,
    fetch
  });

  if (ret && Array.isArray(ret)) {
    return ret;
  }
};

export const getListUserInfo = async (
  uuid: string,
  fetch: typeof window.fetch = window.fetch
) => {
  let ret = await get<UserListInfo>(`/api/user/management/${uuid}`, {
    res_type: ResponseType.Json,
    fetch
  });

  if (ret && typeof ret === 'object') {
    return ret;
  }
};

export const getMailStatus = async (
  fetch: typeof window.fetch = window.fetch
) => {
  return await get<{ active: boolean }>('/api/user/management/mail', {
    res_type: ResponseType.Json,
    fetch
  });
};

export const deleteUser = async (uuid: string) => {
  return await delete_(`/api/user/management`, {
    body: { uuid }
  });
};

export interface CreateUserRequest {
  name: string;
  email: string;
  password?: string;
}

export const createUser = async (data: CreateUserRequest) => {
  return await post<{ uuid: string }>('/api/user/management', {
    body: data,
    res_type: ResponseType.Json
  });
};
