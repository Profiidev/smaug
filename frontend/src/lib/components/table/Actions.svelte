<script lang="ts">
  import Trash from '@lucide/svelte/icons/trash';
  import Pencil from '@lucide/svelte/icons/pencil';
  import { Button } from 'positron-components/components/ui/button';

  interface Props {
    edit_disabled: boolean;
    delete_disabled: boolean;
    editHref?: string;
    remove: () => void;
  }

  let { edit_disabled, delete_disabled, editHref, remove }: Props = $props();
</script>

<div class="flex w-full space-x-4">
  <Button
    size="icon"
    variant="secondary"
    href={editHref}
    disabled={edit_disabled}
    class="ml-auto cursor-pointer"
  >
    <Pencil />
  </Button>
  <Button
    size="icon"
    class="cursor-pointer"
    variant="destructive"
    onclick={remove}
    disabled={delete_disabled}
  >
    <Trash />
  </Button>
</div>
