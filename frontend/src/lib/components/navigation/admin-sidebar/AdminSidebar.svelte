<script lang="ts">
  import * as Sidebar from 'positron-components/components/ui/sidebar';
  import AdminSidebarHeader from './AdminSidebarHeader.svelte';
  import AdminSidebarContent, {
    type AdminNavItem
  } from './AdminSidebarContent.svelte';
  import AdminSidebarUser from './AdminSidebarUser.svelte';
  import type { Snippet } from 'svelte';

  interface Props {
    items: AdminNavItem[];
    children: Snippet;
  }

  const { children, items }: Props = $props();
</script>

<Sidebar.Provider>
  <Sidebar.Root collapsible="icon" variant="floating">
    <Sidebar.Header>
      <AdminSidebarHeader />
    </Sidebar.Header>
    <Sidebar.Content>
      <AdminSidebarContent {items} />
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
    <Sidebar.Trigger
      class="absolute top-5 left-3 flex cursor-pointer md:hidden"
    />
    {@render children()}
  </Sidebar.Inset>
</Sidebar.Provider>
