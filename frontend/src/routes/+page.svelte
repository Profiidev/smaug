<script lang="ts">
  import { dummy } from '../lib/backend/dummy.svelte';
  import { Label } from 'positron-components/components/ui/label';

  let text = dummy();
</script>

{#await text}
  <p>Loading ...</p>
{:then text}
  <p>{text}</p>
{/await}
<Label>Testing</Label>
