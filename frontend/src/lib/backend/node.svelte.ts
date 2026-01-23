import { ResponseType, get, post } from 'positron-components/backend';

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

export interface Node {
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
  let ret = await get<Node[]>('/api/admin/nodes', {
    res_type: ResponseType.Json,
    fetch
  });
  if (Array.isArray(ret)) {
    return ret;
  }
};
