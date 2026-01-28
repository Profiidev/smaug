<script lang="ts">
  import * as Sidebar from 'positron-components/components/ui/sidebar';
  import SidebarHeader from './SidebarHeader.svelte';
  import SidebarContent from './SidebarContent.svelte';
  import SidebarUser from './SidebarUser.svelte';
  import type { Snippet } from 'svelte';
  import type { UserInfo } from '$lib/backend/user.svelte';
  import { items } from './items.svelte';

  interface Props {
    user: UserInfo;
    children: Snippet;
  }

  const { children, user }: Props = $props();
</script>

<Sidebar.Provider>
  <Sidebar.Root collapsible="icon" variant="floating">
    <Sidebar.Header>
      <SidebarHeader />
    </Sidebar.Header>
    <Sidebar.Content>
      <SidebarContent {items} {user} />
    </Sidebar.Content>
    <Sidebar.Footer>
      <SidebarUser name={user.name} email={user.email} avatar="" />
    </Sidebar.Footer>
  </Sidebar.Root>
  <Sidebar.Inset>
    <Sidebar.Trigger
      class="absolute top-5 left-3 flex cursor-pointer md:hidden"
    />
    {@render children()}
  </Sidebar.Inset>
</Sidebar.Provider>
