<script lang="ts">
  import Egg from '@lucide/svelte/icons/egg';

  interface Props {
    name: string;
    href: string;
  }

  let { name, href }: Props = $props();
</script>

<div class="ml-4 flex items-center gap-2.5">
  <div
    class="bg-muted text-muted-foreground flex size-7 flex-none items-center justify-center rounded-lg"
  >
    <Egg class="size-4" />
  </div>
  <a {href} class="font-medium underline-offset-4 hover:underline">{name}</a>
</div>
