import Settings from '@lucide/svelte/icons/settings';
import Server from '@lucide/svelte/icons/server';
import House from '@lucide/svelte/icons/house';
import type { NavItem } from '$lib/components/navigation/sidebar/SidebarContent.svelte';
import { Permission } from '$lib/permissions.svelte';

export const items: NavItem[] = [
  { label: 'Overview', href: '/', icon: House },
  { label: 'Settings', href: '/settings', icon: Settings },
  {
    label: 'Nodes',
    href: '/nodes',
    icon: Server,
    requiredPermission: Permission.NODE_VIEW
  }
];
