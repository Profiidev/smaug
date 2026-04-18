import {
  ResponseType,
  delete_,
  get,
  post,
  put
} from '@profidev/pleiades/backend';

export interface GroupInfo {
  id: string;
  name: string;
  permissions: string[];
  users: SimpleUserInfo[];
}

export interface SimpleUserInfo {
  id: string;
  name: string;
}

export interface GroupListResponse {
  groups: GroupInfo[];
  admin_group?: string;
}

export const listGroups = async (fetch: typeof window.fetch = window.fetch) => {
  const ret = await get<GroupListResponse>('/api/group', {
    fetch,
    res_type: ResponseType.Json
  });

  if (ret && typeof ret === 'object') {
    return ret;
  }
  return undefined;
};

export const getGroupInfo = async (
  uuid: string,
  fetch: typeof window.fetch = window.fetch
) => {
  const ret = await get<GroupInfo>(`/api/group/${uuid}`, {
    fetch,
    res_type: ResponseType.Json
  });

  if (ret && typeof ret === 'object') {
    return ret;
  }
  return undefined;
};

export interface GroupCreateRequest {
  name: string;
}

export interface GroupCreateResponse {
  uuid: string;
}

export const createGroup = async (data: GroupCreateRequest) =>
  await post<GroupCreateResponse>('/api/group', {
    body: data,
    res_type: ResponseType.Json
  });

export interface GroupDeleteRequest {
  uuid: string;
}

export const deleteGroup = async (data: GroupDeleteRequest) =>
  await delete_('/api/group', {
    body: data
  });

export interface GroupEditRequest {
  uuid: string;
  name: string;
  permissions: string[];
  users: string[];
}

export const editGroup = async (data: GroupEditRequest) =>
  await put('/api/group', {
    body: data
  });

export const simpleUserList = async (
  fetch: typeof window.fetch = window.fetch
) => {
  const ret = await get<SimpleUserInfo[]>('/api/group/users', {
    fetch,
    res_type: ResponseType.Json
  });

  if (ret && Array.isArray(ret)) {
    return ret;
  }
  return undefined;
};
