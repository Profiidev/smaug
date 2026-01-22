<script lang="ts">
  import * as Sidebar from 'positron-components/components/ui/sidebar';
  import type { Component } from 'svelte';

  interface Props {
    items: {
      label: string;
      href: string;
      icon?: Component;
    }[];
  }

  const { items }: Props = $props();
</script>

<Sidebar.Group>
  <Sidebar.GroupLabel>General</Sidebar.GroupLabel>
  <Sidebar.Menu>
    {#each items as item}
      <Sidebar.MenuItem>
        <Sidebar.MenuButton tooltipContent={item.label}>
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
