<script lang="ts">
  import { onMount } from 'svelte';
  import { z } from 'zod';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import { CopyButton } from '@profidev/pleiades/components/ui-extra/copy-button';
  import * as Select from '@profidev/pleiades/components/ui/select';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import { toast } from '@profidev/pleiades/components/util/general';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import * as Code from '$lib/components/code';
  import { Permission } from '$lib/permissions.svelte';
  import { dockerCompose, dockerRun } from './code.svelte';
  import {
    nodeTokenRegenerate,
    type NodeInfo,
    type UserInfo
  } from '$lib/client';

  let { data } = $props();

  let node: NodeInfo | undefined = $state();
  let user: UserInfo | undefined = $state();
  let token = $state<string>();
  let setupMethod = $state('Docker Compose');
  let regenerateOpen = $state(false);
  let isLoading = $state(false);
  let readonly = $derived(
    !node || !user?.permissions.includes(Permission.NODE_EDIT)
  );

  onMount(() => {
    // set by the create page, the token can not be fetched again
    let newToken = sessionStorage.getItem('newNodeToken');
    if (newToken) {
      token = newToken;
      sessionStorage.removeItem('newNodeToken');
    }
  });

  $effect(() => {
    data.nodeRes.then((res) => {
      if (!res.data) return;
      node = res.data;
    });
  });

  $effect(() => {
    data.user.then((d) => {
      user = d;
    });
  });

  const regenerateConfirm = async () => {
    isLoading = true;
    let res = await nodeTokenRegenerate({ path: { uuid: data.uuid } });
    isLoading = false;

    if (!res.data) {
      return { error: 'Failed to regenerate token' };
    } else {
      toast.success(`Token for node ${node?.name} regenerated successfully`);
      token = res.data.token;
    }
  };
</script>

<h4 class="mb-2">Node Setup</h4>
<div class="flex w-full flex-col gap-2">
  <Label class="mr-4 text-nowrap"
    >Node Auth Token:
    {#if token}
      <span class="text-destructive">(Can not be viewed again!)</span>
    {/if}
  </Label>
  <div class="flex max-w-141 gap-2">
    {#if token}
      <CopyButton text={token} variant="outline" class="grow justify-start">
        <span class="truncate">{token}</span>
      </CopyButton>
    {:else}
      <Input value="Can not be viewed." readonly class="text-destructive" />
    {/if}
    <Button
      variant="destructive"
      class="cursor-pointer"
      onclick={() => (regenerateOpen = true)}
      disabled={readonly}
    >
      <RotateCcw />
      Regenerate
    </Button>
  </div>
  <Label class="mr-4 text-nowrap">Setup Method:</Label>
  <Select.Root bind:value={setupMethod} type="single" allowDeselect={false}>
    <Select.Trigger class="w-48">
      {setupMethod}
    </Select.Trigger>
    <Select.Content>
      <Select.Group>
        <Select.Item value="Docker Compose">Docker Compose</Select.Item>
        <Select.Item value="Docker Run">Docker Run</Select.Item>
      </Select.Group>
    </Select.Content>
  </Select.Root>
  <Code.Root
    code={setupMethod === 'Docker Compose'
      ? dockerCompose(token ?? '<TOKEN>')
      : dockerRun(token ?? '<TOKEN>')}
    lang={setupMethod === 'Docker Compose' ? 'yaml' : 'bash'}
    class="mt-4 min-w-0 grow"
  >
    <Code.CopyButton />
  </Code.Root>
</div>
<FormDialog
  title={`Regenerate Token`}
  description={`Do you really want to regenerate the token for node ${node?.name}? The wings node has to be reconfigured with the new token.`}
  confirm="Regenerate"
  confirmVariant="destructive"
  onsubmit={regenerateConfirm}
  bind:open={regenerateOpen}
  bind:isLoading
  schema={z.object({})}
/>
