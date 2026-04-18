import type { UserEditRequest, UserListInfo } from '$lib/backend/user.svelte';
import type { FormValue } from '@profidev/pleiades/components/form/types';
import { z } from 'zod';

export const userSettings = z.object({
  groups: z.array(z.string()),
  name: z.string().min(1, 'User name is required')
});

export const reformatData = (
  data: FormValue<typeof userSettings>,
  uuid: string
): UserEditRequest => ({
  groups: data.groups || [],
  name: data.name,
  uuid
});

export const formatData = (
  user: UserListInfo
): FormValue<typeof userSettings> => ({
  groups: user.groups.map((group) => group.uuid),
  name: user.name
});

export const resetPassword = z.object({
  new_password: z.string().min(6, 'Password must be at least 6 characters long')
});
