<script lang="ts">
  import * as Sidebar from 'positron-components/components/ui/sidebar';
  import SidebarHeader from './SidebarHeader.svelte';
  import SidebarContent, { type NavItem } from './SidebarContent.svelte';
  import SidebarUser from './SidebarUser.svelte';
  import type { Snippet } from 'svelte';

  interface Props {
    items: NavItem[];
    children: Snippet;
  }

  const { children, items }: Props = $props();
</script>

<Sidebar.Provider>
  <Sidebar.Root collapsible="icon" variant="floating">
    <Sidebar.Header>
      <SidebarHeader />
    </Sidebar.Header>
    <Sidebar.Content>
      <SidebarContent {items} />
    </Sidebar.Content>
    <Sidebar.Footer>
      <SidebarUser name="John Doe" email="john.doe@example.com" avatar="" />
    </Sidebar.Footer>
  </Sidebar.Root>
  <Sidebar.Inset>
    <Sidebar.Trigger
      class="absolute top-5 left-3 flex cursor-pointer md:hidden"
    />
    {@render children()}
  </Sidebar.Inset>
</Sidebar.Provider>
