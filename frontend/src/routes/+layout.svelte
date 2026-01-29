<script lang="ts">
  import { ModeWatcher } from 'positron-components/components/util/general';
  import { Toaster } from 'positron-components/components/ui/sonner';
  import '../app.css';
  import { connectWebsocket } from '$lib/backend/updater.svelte';
  import { onMount } from 'svelte';
  import { testToken } from '$lib/backend/auth.svelte';
  import { goto } from '$app/navigation';
  import Sidebar from '$lib/components/navigation/sidebar/Sidebar.svelte';
  import { page } from '$app/state';
  import type { UserInfo } from '$lib/backend/user.svelte';
  import { noSidebarPaths } from '$lib/components/navigation/sidebar/items.svelte';
  import { setMode } from 'mode-watcher';

  let { children, data } = $props();

  onMount(() => {
    setMode('dark');
    testToken().then((valid) => {
      // can also be undefined if there was an error
      if (valid === false) {
        if (!noSidebarPaths.includes(page.url.pathname)) {
          goto('/login');
        }
      } else {
        connectWebsocket();
      }
    });
  });
</script>

<ModeWatcher />
<Toaster position="top-right" closeButton={true} richColors={true} />

{#if noSidebarPaths.includes(page.url.pathname)}
  {@render children()}
{:else}
  <Sidebar
    user={data.user ?? {
      email: '',
      name: '',
      permissions: [],
      uuid: ''
    }}
  >
    {@render children()}
  </Sidebar>
{/if}
