<script lang="ts">
  import { ModeWatcher } from 'positron-components/components/util/general';
  import { Toaster } from 'positron-components/components/ui/sonner';
  import '../app.css';
  import { connectWebsocket } from '$lib/backend/updater.svelte';
  import { onMount } from 'svelte';
  import { testToken } from '$lib/backend/auth.svelte';
  import { goto } from '$app/navigation';
  import Sidebar from '$lib/components/navigation/sidebar/Sidebar.svelte';
  import Settings from '@lucide/svelte/icons/settings';
  import Server from '@lucide/svelte/icons/server';
  import House from '@lucide/svelte/icons/house';
  import type { NavItem } from '$lib/components/navigation/sidebar/SidebarContent.svelte';
  import { page } from '$app/state';

  let { children } = $props();

  onMount(() => {
    testToken().then((valid) => {
      // can also be undefined if there was an error
      if (valid === false) {
        goto('/login');
      } else {
        connectWebsocket();
      }
    });
  });

  const items: NavItem[] = [
    { label: 'Overview', href: '/', icon: House },
    { label: 'Settings', href: '/settings', icon: Settings },
    { label: 'Nodes', href: '/nodes', icon: Server }
  ];

  const noSidebar = ['/login', '/setup'];
</script>

<ModeWatcher />
<Toaster position="top-right" closeButton={true} richColors={true} />

{#if noSidebar.includes(page.url.pathname)}
  {@render children()}
{:else}
  <Sidebar {items}>
    {@render children()}
  </Sidebar>
{/if}
