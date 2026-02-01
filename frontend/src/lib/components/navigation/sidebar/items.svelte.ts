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
    label: 'General',
    items: [{ label: 'Overview', href: '/', icon: House }]
  },
  {
    label: 'Servers',
    items: [
      {
        label: 'Nodes',
        href: '/nodes',
        icon: Server,
        requiredPermission: Permission.NODE_VIEW
      }
    ]
  },
  {
    label: 'Administration',
    items: [
      {
        label: 'Users',
        href: '/users',
        icon: User,
        requiredPermission: Permission.USER_VIEW
      },
      {
        label: 'Groups',
        href: '/groups',
        icon: Users,
        requiredPermission: Permission.GROUP_VIEW
      },
      {
        label: 'Settings',
        href: '/settings',
        icon: Settings,
        requiredPermission: Permission.SETTINGS_VIEW
      }
    ]
  }
];

export const noSidebarPaths = [
  '/login',
  '/setup',
  '/password',
  '/password/forgot',
  '/password/reset'
];
