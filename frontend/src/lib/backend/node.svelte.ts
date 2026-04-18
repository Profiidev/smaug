import { ResponseType, delete_, get, post } from '@profidev/pleiades/backend';

export interface CreateNode {
  name: string;
  address: string;
  secure: boolean;
  disk_limit_mb?: number;
  memory_limit_mb?: number;
  cpu_limit?: number;
}

export interface CreateNodeRes {
  uuid: string;
}

export const createNode = async (node: CreateNode) =>
  await post<CreateNodeRes>('/api/nodes', {
    body: node,
    res_type: ResponseType.Json
  });

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
  const ret = await get<NodeInfo[]>('/api/nodes', {
    fetch,
    res_type: ResponseType.Json
  });
  if (Array.isArray(ret)) {
    return ret;
  }
  return undefined;
};

export const deleteNode = async (uuid: string) =>
  await delete_(`/api/nodes`, {
    body: { uuid }
  });

export const nodeInfo = async (
  uuid: string,
  fetch: typeof window.fetch = window.fetch
) =>
  await get<NodeInfo>(`/api/nodes/${uuid}`, {
    fetch,
    res_type: ResponseType.Json
  });

export interface UpdateNode {
  name: string;
  address: string;
  secure: boolean;
  disk_limit_mb?: number;
  memory_limit_mb?: number;
  cpu_limit?: number;
}

export const updateNode = async (uuid: string, node: UpdateNode) =>
  await post(`/api/nodes/${uuid}`, {
    body: node
  });
