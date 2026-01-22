<script lang="ts">
  import * as Sidebar from 'positron-components/components/ui/sidebar';
  import AdminSidebarHeader from './AdminSidebarHeader.svelte';
  import AdminSidebarContent from './AdminSidebarContent.svelte';
  import Settings from '@lucide/svelte/icons/settings';
  import House from '@lucide/svelte/icons/house';
  import AdminSidebarUser from './AdminSidebarUser.svelte';

  const { children } = $props();

  const contentItems = [
    { label: 'Overview', href: '/admin', icon: House },
    { label: 'Settings', href: '/admin/settings', icon: Settings }
  ];
</script>

<Sidebar.Provider>
  <Sidebar.Root collapsible="icon">
    <Sidebar.Header>
      <AdminSidebarHeader />
    </Sidebar.Header>
    <Sidebar.Content>
      <AdminSidebarContent items={contentItems} />
    </Sidebar.Content>
    <Sidebar.Footer>
      <AdminSidebarUser
        name="John Doe"
        email="john.doe@example.com"
        avatar=""
      />
    </Sidebar.Footer>
  </Sidebar.Root>
  <Sidebar.Inset>
    {@render children()}
  </Sidebar.Inset>
</Sidebar.Provider>
