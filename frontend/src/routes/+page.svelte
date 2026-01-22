<script lang="ts">
  import { dummy } from '../lib/backend/dummy.svelte';
  import { Label } from 'positron-components/components/ui/label';
  import { Button } from 'positron-components/components/ui/button';

  let text = dummy();
</script>

{#await text}
  <p>Loading ...</p>
{:then text}
  <p>{text}</p>
{/await}
<Label>Testing</Label>
<Button href="/admin" variant="link">Admin</Button>
