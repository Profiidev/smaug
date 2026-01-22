<script lang="ts">
  import AdminSidebar from '$lib/components/navigation/admin-sidebar/AdminSidebar.svelte';

  const { children } = $props();
</script>

<AdminSidebar>
  {@render children()}
</AdminSidebar>
