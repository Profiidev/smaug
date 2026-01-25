<script lang="ts">
  import FormDialog from 'positron-components/components/form/form-dialog.svelte';
  import type { PageData } from './$types';
  import * as Card from 'positron-components/components/ui/card';
  import { Separator } from 'positron-components/components/ui/separator';
  import * as Tabs from 'positron-components/components/ui/tabs';
  import { z } from 'zod';
  import { deleteNode, updateNode } from '$lib/backend/node.svelte';
  import { toast } from 'positron-components/components/util/general';
  import { goto, invalidate } from '$app/navigation';
  import { Button } from 'positron-components/components/ui/button';
  import Trash from '@lucide/svelte/icons/trash';
  import { reformatData, undoReformatData } from '../create/schema.svelte';
  import GeneralSettings from '../create/GeneralSettings.svelte';
  import AdvancedSettings from '../create/AdvancedSettings.svelte';
  import Save from '@lucide/svelte/icons/save';
  import type { FormRecord } from 'positron-components/components/form/types';
  import { Spinner } from 'positron-components/components/ui/spinner';
  import { RequestError } from 'positron-components/backend';

  interface Props {
    data: PageData;
  }

  const { data }: Props = $props();

  let tab = $state('settings');
  let deleteOpen = $state(false);
  let isLoading = $state(false);

  const deleteItemConfirm = async () => {
    isLoading = true;
    let ret = await deleteNode(data.node.id);
    isLoading = false;

    if (ret) {
      return { error: 'Failed to delete node' };
    } else {
      toast.success(`Node ${data.node.name} deleted successfully`);
      goto('/admin/nodes');
    }
  };

  const saveSettings = async (form: FormRecord) => {
    let rawNode = { ...undoReformatData(data.node), ...form };
    let node = reformatData(rawNode);

    let res = await updateNode(data.node.id, node);

    if (res) {
      if (res === RequestError.Conflict) {
        return { error: 'This node name is already in use', field: 'name' };
        // TODO: use bad request when component supports it
      } else if (res === RequestError.Gone) {
        return { error: 'Invalid node address', field: 'address' };
      } else {
        return { error: 'Failed to update node' };
      }
    } else {
      toast.success(`Node ${data.node.name} updated successfully`);
      // do not trigger form reset
      return { error: '' };
    }
  };
</script>

<div class="p-4">
  <div class="ml-7 flex items-center md:m-0">
    <h3 class="text-xl font-medium">Node: {data.node.name}</h3>
    <Button
      class="ml-auto"
      onclick={() => (deleteOpen = true)}
      variant="destructive"
    >
      <Trash />
      Delete
    </Button>
  </div>
  <Card.Root class="mt-4 py-2">
    <Tabs.Root bind:value={tab}>
      <Card.Header class="px-2">
        <Tabs.List>
          <Tabs.Trigger value="setup">Node Setup</Tabs.Trigger>
          <Tabs.Trigger value="settings">Settings</Tabs.Trigger>
        </Tabs.List>
      </Card.Header>
      <Separator />
      <Card.Content class="px-4 py-2">
        <Tabs.Content value="setup">
          <p>Setup</p>
        </Tabs.Content>
        <Tabs.Content value="settings">
          <div class="grid items-start gap-8 lg:grid-cols-[1fr_auto_1fr]">
            <div>
              <Card.Title class="mb-4">General Settings</Card.Title>
              <GeneralSettings
                onsubmit={saveSettings}
                {isLoading}
                initialValue={undoReformatData(data.node)}
                {footer}
              />
            </div>
            <Separator orientation="horizontal" class="lg:hidden" />
            <Separator orientation="vertical" class="hidden lg:block" />
            <div>
              <Card.Title class="mb-4">Advanced Settings</Card.Title>
              <AdvancedSettings
                onsubmit={saveSettings}
                {isLoading}
                initialValue={undoReformatData(data.node)}
                {footer}
              />
            </div>
          </div>
        </Tabs.Content>
      </Card.Content>
    </Tabs.Root>
  </Card.Root>
</div>
<FormDialog
  title={`Delete Node`}
  description={`Do you really want to delete the node ${data.node.name}?`}
  confirm="Delete"
  confirmVariant="destructive"
  onsubmit={deleteItemConfirm}
  bind:open={deleteOpen}
  bind:isLoading
  schema={z.object({})}
/>

{#snippet footer({ isLoading }: { isLoading: boolean })}
  <div class="mt-4 flex w-full">
    <Button class="ml-auto" type="submit" disabled={isLoading}>
      {#if isLoading}
        <Spinner />
      {:else}
        <Save />
      {/if}
      Save Changes</Button
    >
  </div>
{/snippet}
