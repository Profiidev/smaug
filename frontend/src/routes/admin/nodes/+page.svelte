<script lang="ts">
  import { Button } from 'positron-components/components/ui/button';
  import Plus from '@lucide/svelte/icons/plus';
</script>

<div class="p-4">
  <div class="flex items-center">
    <h3 class="text-xl font-medium">Nodes</h3>
    <Button class="ml-auto" href="/admin/nodes/create">
      <Plus />
      Create
    </Button>
  </div>
</div>
