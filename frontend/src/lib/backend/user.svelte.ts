import type { Permission } from '$lib/permissions.svelte';
import {
  RequestError,
  ResponseType,
  delete_,
  get,
  post,
  put
} from '@profidev/pleiades/backend';
import { fetchKey, getEncrypt } from './auth.svelte';

export interface UserInfo {
  uuid: string;
  name: string;
  email: string;
  permissions: Permission[];
  avatar?: string;
}

export const getUserInfo = async (fetch: typeof window.fetch = window.fetch) =>
  await get<UserInfo>('/api/user/info', {
    fetch,
    res_type: ResponseType.Json
  });

export interface AccountUpdate {
  username: string;
}

export const updateAccount = async (data: AccountUpdate) =>
  await post('/api/user/account/update', {
    body: data
  });

export interface AvatarUpdate {
  avatar: string;
}

export const updateAvatar = async (data: AvatarUpdate) =>
  await post('/api/user/account/avatar', {
    body: data
  });

export interface UpdatePassword {
  old_password: string;
  new_password: string;
}

export const updatePassword = async (payload: UpdatePassword) => {
  const encrypt = getEncrypt();
  if (!encrypt) {
    return RequestError.Other;
  }

  let encrypted_password = encrypt.encrypt(payload.old_password);
  payload.old_password = encrypted_password || '';
  encrypted_password = encrypt.encrypt(payload.new_password);
  payload.new_password = encrypted_password || '';

  const res = await post('/api/user/account/password', {
    body: payload
  });

  if (res === RequestError.Unauthorized) {
    const _ = fetchKey();
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
  const ret = await get<UserListInfo[]>('/api/user/management', {
    fetch,
    res_type: ResponseType.Json
  });

  if (ret && Array.isArray(ret)) {
    return ret;
  }

  return undefined;
};

export type DetailUserInfo = UserListInfo & {
  permissions: Permission[];
};

export const getListUserInfo = async (
  uuid: string,
  fetch: typeof window.fetch = window.fetch
) => {
  const ret = await get<DetailUserInfo>(`/api/user/management/${uuid}`, {
    fetch,
    res_type: ResponseType.Json
  });

  if (ret && typeof ret === 'object') {
    return ret;
  }

  return undefined;
};

export const getMailStatus = async (
  fetch: typeof window.fetch = window.fetch
) => {
  const ret = await get<{ active: boolean }>('/api/user/management/mail', {
    fetch,
    res_type: ResponseType.Json
  });

  if (ret && typeof ret === 'object') {
    return ret;
  }
  return undefined;
};

export const deleteUser = async (uuid: string) =>
  await delete_(`/api/user/management`, {
    body: { uuid }
  });

export interface CreateUserRequest {
  name: string;
  email: string;
  password?: string;
}

export const createUser = async (data: CreateUserRequest) => {
  const encrypt = getEncrypt();
  if (!encrypt) {
    return RequestError.Other;
  }

  const encrypted_password = encrypt.encrypt(data.password || '');
  data.password = encrypted_password || '';

  const res = await post<{ uuid: string }>('/api/user/management', {
    body: data,
    res_type: ResponseType.Json
  });

  if (res === RequestError.Unauthorized) {
    const _ = fetchKey();
  }

  return res;
};

export const simpleGroupList = async (
  fetch: typeof window.fetch = window.fetch
) => {
  const ret = await get<SimpleGroupInfo[]>('/api/user/management/groups', {
    fetch,
    res_type: ResponseType.Json
  });

  if (ret && Array.isArray(ret)) {
    return ret;
  }
  return undefined;
};

export interface UserEditRequest {
  uuid: string;
  name: string;
  groups: string[];
}

export const editUser = async (data: UserEditRequest) =>
  await put(`/api/user/management`, {
    body: data
  });

export const resetUserAvatar = async (uuid: string) =>
  await delete_(`/api/user/management/avatar`, {
    body: { uuid }
  });

export interface ResetUserPasswordRequest {
  uuid: string;
  new_password: string;
}

export const resetUserPassword = async (data: ResetUserPasswordRequest) => {
  const encrypt = getEncrypt();
  if (!encrypt) {
    return RequestError.Other;
  }

  const encrypted_password = encrypt.encrypt(data.new_password);
  data.new_password = encrypted_password || '';

  const res = await put('/api/user/management/password', {
    body: data
  });

  if (res === RequestError.Unauthorized) {
    const _ = fetchKey();
  }

  return res;
};
