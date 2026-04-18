<script lang="ts">
  import { CopyButton } from '@profidev/pleiades/components/ui-extra/copy-button';
  import { cn } from '@profidev/pleiades/utils';
  import { useCodeCopyButton } from './code.svelte.js';
  import type { CodeCopyButtonProps } from './types.js';

  let {
    ref = $bindable(null),
    variant = 'ghost',
    size = 'icon',
    class: className,
    ...rest
  }: CodeCopyButtonProps = $props();

  const copyButton = useCodeCopyButton();
</script>

<CopyButton
  class={cn('absolute top-2 right-2', className)}
  text={copyButton.code}
  {variant}
  {size}
  {...rest}
/>
