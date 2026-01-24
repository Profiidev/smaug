<script lang="ts">
  import { ModeWatcher } from 'positron-components/components/util/general';
  import { Toaster } from 'positron-components/components/ui/sonner';
  import '../app.css';
  import { connectWebsocket } from '$lib/backend/updater.svelte';

  let { children } = $props();

  connectWebsocket();
</script>

<ModeWatcher />
<Toaster position="top-right" closeButton={true} richColors={true} />

<div class="h-full w-full">
  {@render children()}
</div>
