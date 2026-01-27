<script lang="ts">
  import { page } from '$app/state';
  import * as Sidebar from 'positron-components/components/ui/sidebar';
  import type { Component } from 'svelte';

  export interface NavItem {
    label: string;
    href: string;
    icon?: Component;
  }

  interface Props {
    items: NavItem[];
  }

  const { items }: Props = $props();

  let current = $derived<NavItem | undefined>(
    items
      .filter((item) => page.url.pathname.startsWith(item.href))
      .sort((a, b) => b.href.length - a.href.length)[0] ?? undefined
  );
</script>

<Sidebar.Group>
  <Sidebar.GroupLabel>General</Sidebar.GroupLabel>
  <Sidebar.Menu>
    {#each items as item}
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
