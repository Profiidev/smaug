import { ResponseType, delete_, get, post } from 'positron-components/backend';

export interface CreateNode {
  name: string;
  address: string;
  secure: boolean;
  disk_limit_mb?: number;
  memory_limit_mb?: number;
  cpu_limit?: number;
}

export const createNode = async (node: CreateNode) => {
  return await post<undefined>('/api/admin/nodes', {
    body: node
  });
};

export interface NodeInfo {
  id: string;
  name: string;
  address: string;
  port: number;
  secure: boolean;
  disk_limit_mb?: number;
  memory_limit_mb?: number;
  cpu_limit?: number;
  token: string;
  connected: boolean;
}

export const listNodes = async (fetch: typeof window.fetch = window.fetch) => {
  let ret = await get<NodeInfo[]>('/api/admin/nodes', {
    res_type: ResponseType.Json,
    fetch
  });
  if (Array.isArray(ret)) {
    return ret;
  }
};

export const deleteNode = async (uuid: string) => {
  return await delete_(`/api/admin/nodes`, {
    body: { uuid }
  });
};

export const nodeInfo = async (
  uuid: string,
  fetch: typeof window.fetch = window.fetch
) => {
  return await get<NodeInfo>(`/api/admin/nodes/${uuid}`, {
    res_type: ResponseType.Json,
    fetch
  });
};

export interface UpdateNode {
  name: string;
  address: string;
  secure: boolean;
  disk_limit_mb?: number;
  memory_limit_mb?: number;
  cpu_limit?: number;
}

export const updateNode = async (uuid: string, node: UpdateNode) => {
  return await post<undefined>(`/api/admin/nodes/${uuid}`, {
    body: node
  });
};
