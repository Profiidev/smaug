<script lang="ts">
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Card from '@profidev/pleiades/components/ui/card';
  import * as Tabs from '@profidev/pleiades/components/ui/tabs';
  import { Skeleton } from '@profidev/pleiades/components/ui/skeleton';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import Trash from '@lucide/svelte/icons/trash';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import BaseForm from '@profidev/pleiades/components/form/base-form.svelte';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { goto } from '$app/navigation';
  import { z } from 'zod';
  import { Permission } from '$lib/permissions.svelte';
  import {
    deleteNode,
    updateNode,
    type NodeInfo,
    type UserInfo
  } from '$lib/client';
  import Status from '$lib/components/table/Status.svelte';
  import SaveBar from '$lib/components/form/SaveBar.svelte';
  import SectionErrorDot from '$lib/components/form/SectionErrorDot.svelte';
  import ApplyErrors from '$lib/components/form/ApplyErrors.svelte';
  import {
    DELETE_DESCRIPTION,
    deleteError,
    nodeSettings,
    reformatData,
    SECTION_KEYS,
    stageForm,
    undoReformatData,
    type NodeForm
  } from '../schema.svelte';
  import GeneralSettingsFields from '../GeneralSettingsFields.svelte';
  import AdvancedSettingsFields from '../AdvancedSettingsFields.svelte';
  import SetupTab from './SetupTab.svelte';

  const { data } = $props();

  const sections = [
    {
      description: 'Name, address and connection security.',
      title: 'General',
      value: 'general'
    },
    {
      description: 'CPU, memory and storage the node offers to servers.',
      title: 'Resources',
      value: 'resources'
    }
  ] as const;

  let node: NodeInfo | undefined = $state.raw();
  let user: UserInfo | undefined = $state();
  let saved: NodeForm | undefined = $state.raw();
  let saving = $state(false);
  let saveFailed = $state(false);
  let dirty = $state(false);
  let fieldErrors: Record<string, string> | undefined = $state.raw();
  let deleteOpen = $state(false);
  let isLoading = $state(false);
  // svelte-ignore state_referenced_locally
  let tab = $state(
    sections.some((s) => s.value === data.tab) ? data.tab! : 'setup'
  );

  let readonly = $derived(
    !node || !user?.permissions.includes(Permission.NODE_EDIT)
  );

  $effect(() => {
    data.nodeRes.then((res) => {
      if (!res.data) {
        goto(
          res.response?.status === 404
            ? '/nodes?error=not_found'
            : '/nodes?error=other'
        );
        return;
      }
      node = res.data;
      // A reload (websocket update) must not wipe unsaved edits.
      if (!dirty || !saved) saved = undoReformatData(res.data);
    });
  });

  $effect(() => {
    data.user.then((d) => {
      user = d;
    });
  });

  const onsubmit = async (values: NodeForm) => {
    const res = await updateNode({
      body: reformatData(values),
      path: { uuid: data.uuid }
    });

    if (!res.response?.ok) {
      const status = res.response?.status;
      const field: Record<string, string> | undefined =
        status === 409
          ? { name: 'This node name is already in use' }
          : status === 400
            ? { address: 'Invalid node address' }
            : undefined;
      if (!field) {
        saveFailed = true;
        return { error: 'Failed to update node' };
      }
      saveFailed = false;
      tab = 'general';
      // Applied after BaseForm finished handling this submit.
      setTimeout(() => (fieldErrors = field));
      toast.error("Couldn't save node", {
        description: Object.values(field)[0]
      });
      return { error: '' };
    }

    saveFailed = false;
    saved = values;
    if (node) node = { ...node, name: values.name };
    toast.success(`Node ${values.name} updated successfully`);
    // do not trigger form reset
    return { error: '' };
  };

  const deleteItemConfirm = async () => {
    isLoading = true;
    const res = await deleteNode({ body: { uuid: data.uuid } });
    isLoading = false;

    if (!res.response?.ok) {
      return { error: deleteError(res.response?.status) };
    }

    toast.success(`Node ${node?.name} deleted successfully`);
    setTimeout(() => {
      goto('/nodes');
    });
  };
</script>

<div class="flex h-full max-h-screen min-h-0 w-full flex-col space-y-6 p-4">
  <div class="mt-1! mb-0 ml-7 flex flex-wrap items-center gap-y-2 md:m-0">
    <Button
      size="icon"
      variant="ghost"
      href="/nodes"
      class="mr-2 cursor-pointer"
      aria-label="Back to nodes"
    >
      <ArrowLeft class="size-5" />
    </Button>
    <h3 class="flex text-xl font-medium">
      Node:
      {#if !node}
        <Skeleton class="ml-2 h-7 w-20" />
      {:else}
        {node.name}
      {/if}
    </h3>
    <Status connected={node?.connected} class="mt-0" />
    <Button
      class="ml-auto cursor-pointer"
      onclick={() => (deleteOpen = true)}
      variant="destructive"
      disabled={readonly}
    >
      <Trash />
      Delete
    </Button>
  </div>
  <Separator class="my-4" />
  <BaseForm
    class="flex min-h-0 grow flex-col gap-4"
    schema={nodeSettings}
    {onsubmit}
    bind:isLoading={saving}
  >
    {#snippet children({ props })}
      {@const disabled = props.disabled || readonly}
      <ApplyErrors formData={props.formData} errors={fieldErrors} />
      <Tabs.Root
        bind:value={tab}
        class="mx-auto flex min-h-0 w-full max-w-4xl grow flex-col gap-4"
      >
        <Tabs.List class="w-full shrink-0 overflow-x-auto overflow-y-hidden">
          <Tabs.Trigger value="setup" class="cursor-pointer">Setup</Tabs.Trigger
          >
          {#each sections as section (section.value)}
            <Tabs.Trigger value={section.value} class="cursor-pointer">
              {section.title}
              <SectionErrorDot
                formData={props.formData}
                keys={SECTION_KEYS[section.value]}
              />
            </Tabs.Trigger>
          {/each}
        </Tabs.List>
        <Tabs.Content
          value="setup"
          class="min-h-0 overflow-y-auto data-[state=inactive]:hidden"
        >
          <SetupTab uuid={data.uuid} name={node?.name} {readonly} />
        </Tabs.Content>
        {#each sections as section (section.value)}
          <Tabs.Content
            value={section.value}
            class="flex min-h-0 flex-col data-[state=inactive]:hidden"
          >
            <Card.Root class="min-h-0">
              <Card.Header>
                <Card.Title>{section.title}</Card.Title>
                <Card.Description>{section.description}</Card.Description>
              </Card.Header>
              <Card.Content class="min-h-0 overflow-y-auto">
                <div class="grid gap-4">
                  {#if section.value === 'general'}
                    <GeneralSettingsFields
                      formData={stageForm(props.formData)}
                      {disabled}
                    />
                  {:else}
                    <AdvancedSettingsFields
                      formData={stageForm(props.formData)}
                      {disabled}
                    />
                  {/if}
                </div>
              </Card.Content>
            </Card.Root>
          </Tabs.Content>
        {/each}
      </Tabs.Root>
      <!-- Hidden, not unmounted: remounting would reset unsaved edits. -->
      <div class={tab === 'setup' ? 'hidden' : 'contents'}>
        <SaveBar
          formData={props.formData}
          {saved}
          isLoading={saving}
          isError={saveFailed}
          {readonly}
          bind:dirty
        />
      </div>
    {/snippet}
    {#snippet footer()}{/snippet}
  </BaseForm>
</div>

<FormDialog
  title={`Delete node ${node?.name}?`}
  description={DELETE_DESCRIPTION}
  confirm="Delete"
  confirmVariant="destructive"
  onsubmit={deleteItemConfirm}
  bind:open={deleteOpen}
  bind:isLoading
  schema={z.object({})}
/>
