<script lang="ts">
  import AdminSidebar from '$lib/components/navigation/admin-sidebar/AdminSidebar.svelte';
  import Settings from '@lucide/svelte/icons/settings';
  import Server from '@lucide/svelte/icons/server';
  import House from '@lucide/svelte/icons/house';
  import type { AdminNavItem } from '$lib/components/navigation/admin-sidebar/AdminSidebarContent.svelte';

  const { children } = $props();

  const items: AdminNavItem[] = [
    { label: 'Overview', href: '/admin', icon: House },
    { label: 'Settings', href: '/admin/settings', icon: Settings },
    { label: 'Nodes', href: '/admin/nodes', icon: Server }
  ];
</script>

<AdminSidebar {items}>
  {@render children()}
</AdminSidebar>
