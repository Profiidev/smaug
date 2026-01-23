import {
  RequestError,
  ResponseType,
  get,
  post
} from 'positron-components/backend';

export const dummy = async () => {
  let res = await get<string>('/api/test', ResponseType.Text);
  if (!Object.values(RequestError).includes(res as RequestError)) {
    return res;
  }
};

export interface CreateNode {
  name: string;
  address: string;
  secure: boolean;
  disk_limit_mb?: number;
  memory_limit_mb?: number;
  cpu_limit?: number;
}

export const createNode = async (node: CreateNode) => {
  return await post<undefined>('/api/admin/nodes', ResponseType.None, node);
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
}

export const listNodes = async () => {
  let ret = await get<Node[]>('/api/admin/nodes', ResponseType.Json);
  if (Array.isArray(ret)) {
    return ret;
  }
};
