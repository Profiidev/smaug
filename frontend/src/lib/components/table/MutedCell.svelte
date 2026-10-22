<script lang="ts">
  import { cn } from '@profidev/pleiades/utils';

  interface Props {
    text?: string | null;
    class?: string;
    /** Tooltip, defaults to `text`. */
    title?: string;
  }

  let { text, class: className, title }: Props = $props();
</script>

<span
  class={cn('text-muted-foreground ml-4', className)}
  title={title ?? text ?? undefined}
>
  {text ?? ''}
</span>
