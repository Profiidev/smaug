<script lang="ts">
  import { page } from '$app/state';
  import type { UserInfo } from '$lib/backend/user.svelte';
  import * as Sidebar from 'positron-components/components/ui/sidebar';
  import type { NavGroup, NavItem } from './items.svelte';

  interface Props {
    items: NavGroup[];
    user: UserInfo;
  }

  const { items, user }: Props = $props();

  let filteredItems = $derived(
    items
      .map((group) => {
        group.items = group.items.filter((item) => {
          if (item.requiredPermission) {
            return user.permissions.includes(item.requiredPermission);
          }
          return true;
        });
        return group;
      })
      .filter((item) => item.items.length > 0)
  );

  let current = $derived<NavItem | undefined>(
    filteredItems
      .flatMap((group) => group.items)
      .filter((item) => page.url.pathname.startsWith(item.href))
      .sort((a, b) => b.href.length - a.href.length)[0] ?? undefined
  );
</script>

{#each filteredItems as group}
  <Sidebar.Group>
    <Sidebar.GroupLabel>General</Sidebar.GroupLabel>
    <Sidebar.Menu>
      {#each group.items as item}
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
{/each}
