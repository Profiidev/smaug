<script lang="ts">
  import { cn } from '@profidev/pleiades/utils';

  interface Props {
    text?: string | null;
    class?: string;
  }

  let { text, class: className }: Props = $props();
</script>

<span
  class={cn('text-muted-foreground ml-4', className)}
  title={text ?? undefined}
>
  {text ?? ''}
</span>
