<script lang="ts">
  import * as Sidebar from 'positron-components/components/ui/sidebar';
  import Dragon from '$lib/components/icons/Dragon.svelte';
  import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open';
  import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';

  // @ts-ignore this is injected at build time via Vite's define option
  let version = __version__;

  let sidebar = Sidebar.useSidebar();
  let isOpen = $derived(sidebar.props.open());
</script>

<Sidebar.Menu>
  <Sidebar.MenuItem class="flex flex-row">
    <Sidebar.MenuButton
      size="lg"
      class="overflow-hidden transition-all ease-linear data-[open=true]:max-w-54 md:max-w-0 md:data-[open=true]:max-w-42"
      data-open={isOpen}
    >
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
    <Sidebar.MenuButton
      size="lg"
      class="ml-auto size-12 cursor-pointer"
      onclick={sidebar.toggle}
      aria-label="Open/Close Sidebar"
      tooltipContent="Smaug"
    >
      <div
        data-open={!isOpen}
        class="data-[open=true]:bg-sidebar-primary data-[open=true]:text-sidebar-primary-foreground flex aspect-square size-8 items-center justify-center rounded-lg"
      >
        {#if isOpen}
          <PanelLeftClose />
        {:else}
          <PanelLeftOpen />
        {/if}
      </div>
    </Sidebar.MenuButton>
  </Sidebar.MenuItem>
</Sidebar.Menu>
