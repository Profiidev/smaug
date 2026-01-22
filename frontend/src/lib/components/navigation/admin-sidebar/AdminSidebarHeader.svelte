<script lang="ts">
  import * as Sidebar from 'positron-components/components/ui/sidebar';
  import Dragon from '$lib/components/icons/Dragon.svelte';

  // @ts-ignore this is injected at build time via Vite's define option
  let version = __version__;
</script>

<Sidebar.Menu>
  <Sidebar.MenuItem>
    <Sidebar.MenuButton size="lg">
      {#snippet child({ props })}
        <a href="/admin" {...props}>
          <div
            class="bg-sidebar-primary text-sidebar-primary-foreground flex aspect-square size-8 items-center justify-center rounded-lg"
          >
            <Dragon class="size-4" />
          </div>
          <div class="flex flex-col gap-0.5 leading-none">
            <span class="font-medium text-nowrap">Smaug Admin</span>
            <span class="text-nowrap">v{version}</span>
          </div>
        </a>
      {/snippet}
    </Sidebar.MenuButton>
  </Sidebar.MenuItem>
</Sidebar.Menu>
