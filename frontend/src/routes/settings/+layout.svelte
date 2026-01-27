<script lang="ts">
  import { Separator } from 'positron-components/components/ui/separator';
  import SimpleSidebar from 'positron-components/components/nav/simple-sidebar.svelte';

  const { children } = $props();

  const routes = [
    {
      title: 'User',
      href: '/settings/user'
    },
    {
      title: 'Mail',
      href: '/settings/mail'
    }
  ];
</script>

<div class="w-full space-y-6 p-4">
  <div class="ml-7 flex items-center md:m-0">
    <h3 class="text-xl font-medium">Settings</h3>
  </div>
  <Separator class="my-6" />
  <div class="flex flex-col space-y-8 lg:flex-row lg:space-y-0 lg:space-x-12">
    <aside class="lg:w-40">
      <SimpleSidebar items={routes} class="" />
    </aside>
    <div class="flex-1">
      {@render children()}
    </div>
  </div>
</div>
