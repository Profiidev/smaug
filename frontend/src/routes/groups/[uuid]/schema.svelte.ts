import type { GroupEditRequest, GroupInfo } from '$lib/backend/groups.svelte';
import type { FormValue } from '@profidev/pleiades/components/form/types';
import { z } from 'zod';

export const groupSettings = z.object({
  group_edit: z.boolean().default(false),
  group_view: z.boolean().default(false),
  name: z.string().min(1, 'Group name is required'),
  node_edit: z.boolean().default(false),
  node_view: z.boolean().default(false),
  settings_edit: z.boolean().default(false),
  settings_view: z.boolean().default(false),
  user_edit: z.boolean().default(false),
  user_view: z.boolean().default(false),
  users: z.array(z.string())
});

export const reformatData = (
  data: FormValue<typeof groupSettings>,
  uuid: string
): GroupEditRequest => {
  const permissions: string[] = [];

  for (const [key, value] of Object.entries(data)) {
    if (key !== 'name' && value === true) {
      permissions.push(key.replace('_', ':'));
    }
  }

  return {
    name: data.name,
    permissions,
    users: data.users || [],
    uuid
  };
};

export const formatData = (
  group: GroupInfo
): FormValue<typeof groupSettings> => {
  const formattedData: FormValue<typeof groupSettings> = {
    group_edit: false,
    group_view: false,
    name: group.name,
    node_edit: false,
    node_view: false,
    settings_edit: false,
    settings_view: false,
    user_edit: false,
    user_view: false,
    users: group.users.map((user) => user.id)
  };

  for (const permission of group.permissions) {
    // oxlint-disable-next-line no-unsafe-type-assertion
    const key = permission.replace(':', '_') as keyof FormValue<
      typeof groupSettings
    >;
    // @ts-ignore
    formattedData[key] = true;
  }

  return formattedData;
};
