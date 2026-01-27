<script lang="ts">
  import { page } from '$app/state';
  import type { UserInfo } from '$lib/backend/user.svelte';
  import type { Permission } from '$lib/permissions.svelte';
  import * as Sidebar from 'positron-components/components/ui/sidebar';
  import type { Component } from 'svelte';

  export interface NavItem {
    label: string;
    href: string;
    icon?: Component;
    requiredPermission?: Permission;
  }

  interface Props {
    items: NavItem[];
    user: UserInfo;
  }

  const { items, user }: Props = $props();

  let filteredItems = $derived(
    items.filter((item) => {
      if (item.requiredPermission) {
        return user.permissions.includes(item.requiredPermission);
      }
      return true;
    })
  );
  let current = $derived<NavItem | undefined>(
    filteredItems
      .filter((item) => page.url.pathname.startsWith(item.href))
      .sort((a, b) => b.href.length - a.href.length)[0] ?? undefined
  );
</script>

<Sidebar.Group>
  <Sidebar.GroupLabel>General</Sidebar.GroupLabel>
  <Sidebar.Menu>
    {#each filteredItems as item}
      <Sidebar.MenuItem>
        <Sidebar.MenuButton
          tooltipContent={item.label}
          class={item.href === current?.href ? 'bg-muted' : ''}
        >
          {#snippet child({ props })}
            <a href={item.href} {...props}>
              {#if item.icon}
                <item.icon />
              {/if}
              <span>{item.label}</span>
            </a>
          {/snippet}
        </Sidebar.MenuButton>
      </Sidebar.MenuItem>
    {/each}
  </Sidebar.Menu>
</Sidebar.Group>
