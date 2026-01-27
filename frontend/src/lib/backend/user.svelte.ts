import type { Permission } from '$lib/permissions.svelte';
import { get, ResponseType } from 'positron-components/backend';

export interface UserInfo {
  uuid: string;
  name: string;
  email: string;
  permissions: Permission[];
}

export const getUserInfo = async (
  fetch: typeof window.fetch = window.fetch
) => {
  let ret = await get<UserInfo>('/api/user', {
    res_type: ResponseType.Json,
    fetch
  });

  if (typeof ret === 'object') {
    return ret;
  }
};
