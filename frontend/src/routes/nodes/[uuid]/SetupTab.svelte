<script lang="ts">
  import { onMount } from 'svelte';
  import { z } from 'zod';
  import { Label } from '@profidev/pleiades/components/ui/label';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Input } from '@profidev/pleiades/components/ui/input';
  import * as Card from '@profidev/pleiades/components/ui/card';
  import * as Select from '@profidev/pleiades/components/ui/select';
  import { CopyButton } from '@profidev/pleiades/components/ui-extra/copy-button';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import { toast } from '@profidev/pleiades/components/util/general';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import * as Code from '$lib/components/code';
  import { nodeTokenRegenerate } from '$lib/client';
  import { dockerCompose, dockerRun } from './code.svelte';

  interface Props {
    uuid: string;
    /** Undefined while the node loads. */
    name?: string;
    readonly: boolean;
  }

  let { uuid, name, readonly }: Props = $props();

  let token = $state<string>();
  let setupMethod = $state('Docker Compose');
  let regenerateOpen = $state(false);
  let isLoading = $state(false);

  onMount(() => {
    // Set by the create page; the token can not be fetched again.
    const newToken = sessionStorage.getItem('newNodeToken');
    if (newToken) {
      token = newToken;
      sessionStorage.removeItem('newNodeToken');
    }
  });

  const regenerateConfirm = async () => {
    isLoading = true;
    const res = await nodeTokenRegenerate({ path: { uuid } });
    isLoading = false;

    if (!res.data) {
      return { error: 'Failed to regenerate token' };
    }
    toast.success(`Token for node ${name} regenerated successfully`);
    token = res.data.token;
  };
</script>

<Card.Root>
  <Card.Header>
    <Card.Title>Setup</Card.Title>
    <Card.Description>
      Run Wings on the node with this token to connect it.
    </Card.Description>
  </Card.Header>
  <Card.Content class="flex flex-col gap-2">
    <Label>
      Node Auth Token:
      {#if token}
        <span class="text-destructive">(Can not be viewed again!)</span>
      {/if}
    </Label>
    <div class="flex max-w-141 gap-2">
      {#if token}
        <CopyButton
          text={token}
          variant="outline"
          class="min-w-0 grow cursor-pointer justify-start"
        >
          <span class="truncate">{token}</span>
        </CopyButton>
      {:else}
        <Input
          value="Can not be viewed."
          readonly
          aria-label="Node Auth Token"
          class="text-destructive"
        />
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
    <Label class="mt-2">Setup Method:</Label>
    <Select.Root bind:value={setupMethod} type="single" allowDeselect={false}>
      <Select.Trigger class="w-48 cursor-pointer">
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
  </Card.Content>
</Card.Root>

<FormDialog
  title={`Regenerate token for ${name}?`}
  description="The current token stops working. Wings on the node has to be reconfigured with the new token."
  confirm="Regenerate"
  confirmVariant="destructive"
  onsubmit={regenerateConfirm}
  bind:open={regenerateOpen}
  bind:isLoading
  schema={z.object({})}
/>
