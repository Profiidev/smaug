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

  let { children, data } = $props();

  onMount(() => {
    testToken().then((valid) => {
      // can also be undefined if there was an error
      if (valid === false) {
        goto('/login');
      } else {
        connectWebsocket();
      }
    });
  });

  const noSidebar = ['/login', '/setup'];
</script>

<ModeWatcher />
<Toaster position="top-right" closeButton={true} richColors={true} />

{#if noSidebar.includes(page.url.pathname)}
  {@render children()}
{:else}
  <Sidebar user={data.user as UserInfo}>
    {@render children()}
  </Sidebar>
{/if}
