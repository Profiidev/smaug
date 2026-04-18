import Settings from '@lucide/svelte/icons/settings';
import Server from '@lucide/svelte/icons/server';
import House from '@lucide/svelte/icons/house';
import { Permission } from '$lib/permissions.svelte';
import type { Component } from 'svelte';
import Users from '@lucide/svelte/icons/users';
import User from '@lucide/svelte/icons/user';

export interface NavGroup {
  label: string;
  items: NavItem[];
}

export interface NavItem {
  label: string;
  href: string;
  icon?: Component;
  requiredPermission?: Permission;
}

export const items: NavGroup[] = [
  {
    items: [{ href: '/', icon: House, label: 'Overview' }],
    label: 'General'
  },
  {
    items: [
      {
        href: '/nodes',
        icon: Server,
        label: 'Nodes',
        requiredPermission: Permission.NODE_VIEW
      }
    ],
    label: 'Servers'
  },
  {
    items: [
      {
        href: '/users',
        icon: User,
        label: 'Users',
        requiredPermission: Permission.USER_VIEW
      },
      {
        href: '/groups',
        icon: Users,
        label: 'Groups',
        requiredPermission: Permission.GROUP_VIEW
      },
      {
        href: '/settings',
        icon: Settings,
        label: 'Settings',
        requiredPermission: Permission.SETTINGS_VIEW
      }
    ],
    label: 'Administration'
  }
];

export const noSidebarPaths = [
  '/login',
  '/setup',
  '/password',
  '/password/forgot',
  '/password/reset'
];
