<script lang="ts">
  import { Badge } from '@profidev/pleiades/components/ui/badge';
  import { cn } from '@profidev/pleiades/utils';
  import { STATUS, type ServerStatus } from './status';

  interface Props {
    status: ServerStatus;
    class?: string;
  }

  let { status, class: className }: Props = $props();

  let info = $derived(STATUS[status]);
</script>

<Badge variant="outline" class={cn('gap-1.5', className)}>
  <span
    class="size-2 shrink-0 rounded-full"
    style:background-color={info.color}
    aria-hidden="true"
  ></span>
  {info.label}
</Badge>
