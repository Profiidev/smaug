<script lang="ts">
  import Server from '@lucide/svelte/icons/server';

  interface Props {
    name: string;
    href: string;
    /** Defaults to Server (the nodes list); servers pass Container. */
    icon?: typeof Server;
  }

  let { name, href, icon: Icon = Server }: Props = $props();
</script>

<div class="ml-4 flex items-center gap-2.5">
  <div
    class="bg-muted text-muted-foreground flex size-7 flex-none items-center justify-center rounded-lg"
  >
    <Icon class="size-4" />
  </div>
  <a {href} class="font-medium underline-offset-4 hover:underline">{name}</a>
</div>
