import type { ColumnDef } from '@tanstack/table-core';
import * as DataTable from 'positron-components/components/ui/data-table';
import { createColumn } from 'positron-components/components/table/helpers.svelte';
import Actions from '$lib/components/table/Actions.svelte';
import { Permission } from '$lib/permissions.svelte';
import {
  type SimpleGroupInfo,
  type UserInfo,
  type UserListInfo
} from '$lib/backend/user.svelte';
import SimpleAvatar from 'positron-components/components/util/simple-avatar.svelte';

export const columns = ({
  deleteUser,
  user
}: {
  deleteUser: (user: UserListInfo) => void;
  user?: UserInfo;
}): ColumnDef<UserListInfo>[] => [
  {
    accessorKey: 'avatar',
    header: () => {},
    cell: ({ row }) => {
      return DataTable.renderComponent(SimpleAvatar, {
        src: row.getValue('avatar') as string,
        class: 'size-8'
      });
    },
    size: 10
  },
  createColumn('name', 'Name'),
  createColumn('email', 'Email'),
  createColumn(
    'groups',
    'Groups',
    (groups: SimpleGroupInfo[]) =>
      groups.map((u) => u.name).join(', ') || 'No Groups'
  ),
  createColumn('uuid', 'UUID'),
  {
    accessorKey: 'actions',
    header: () => {},
    cell: ({ row }) => {
      let disabled = !user?.permissions.includes(Permission.USER_EDIT);

      return DataTable.renderComponent(Actions, {
        edit_disabled: disabled,
        delete_disabled: disabled,
        editHref: `/users/${row.original.uuid}`,
        remove: () => deleteUser(row.original)
      });
    },
    enableHiding: false
  }
];
