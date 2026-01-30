import {
  delete_,
  get,
  post,
  put,
  ResponseType
} from 'positron-components/backend';

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
  let ret = await get<GroupListResponse>('/api/group', {
    res_type: ResponseType.Json,
    fetch
  });

  if (ret && typeof ret === 'object') {
    return ret;
  }
};

export const getGroupInfo = async (
  uuid: string,
  fetch: typeof window.fetch = window.fetch
) => {
  let ret = await get<GroupInfo>(`/api/group/${uuid}`, {
    res_type: ResponseType.Json,
    fetch
  });

  if (ret && typeof ret === 'object') {
    return ret;
  }
};

export interface GroupCreateRequest {
  name: string;
}

export interface GroupCreateResponse {
  uuid: string;
}

export const createGroup = async (data: GroupCreateRequest) => {
  return await post<GroupCreateResponse>('/api/group', {
    body: data,
    res_type: ResponseType.Json
  });
};

export interface GroupDeleteRequest {
  uuid: string;
}

export const deleteGroup = async (data: GroupDeleteRequest) => {
  return await delete_('/api/group', {
    body: data
  });
};

export interface GroupEditRequest {
  uuid: string;
  name: string;
  permissions: string[];
  users: string[];
}

export const editGroup = async (data: GroupEditRequest) => {
  return await put<void>('/api/group', {
    body: data
  });
};
