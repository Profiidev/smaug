<script lang="ts">
  // Renders the group Permissions component inside a real BaseForm (superForm).
  import BaseForm from '@profidev/pleiades/components/form/base-form.svelte';
  import type { UserInfo } from '$lib/client';
  import Permissions from '$routes/groups/[uuid]/Permissions.svelte';
  import { groupSettings } from '$routes/groups/[uuid]/schema.svelte';

  interface Props {
    user?: UserInfo;
    readonly?: boolean;
  }

  let { user, readonly = false }: Props = $props();
</script>

<BaseForm schema={groupSettings} onsubmit={() => undefined}>
  {#snippet children({ props })}
    <Permissions {user} {readonly} {...props} />
  {/snippet}
  {#snippet footer()}{/snippet}
</BaseForm>
