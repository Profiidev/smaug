<script lang="ts">
  import { ModeWatcher } from 'positron-components/components/util/general';
  import { Toaster } from 'positron-components/components/ui/sonner';
  import '../app.css';
  import { connectWebsocket } from '$lib/backend/updater.svelte';
  import { onMount } from 'svelte';
  import { testToken } from '$lib/backend/auth.svelte';
  import { goto } from '$app/navigation';

  let { children } = $props();

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
</script>

<ModeWatcher />
<Toaster position="top-right" closeButton={true} richColors={true} />

<div class="h-full w-full">
  {@render children()}
</div>
