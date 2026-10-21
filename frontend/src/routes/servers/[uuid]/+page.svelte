<script lang="ts">
  import { fromStore } from 'svelte/store';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import * as Card from '@profidev/pleiades/components/ui/card';
  import * as Tabs from '@profidev/pleiades/components/ui/tabs';
  import { Skeleton } from '@profidev/pleiades/components/ui/skeleton';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import Info from '@lucide/svelte/icons/info';
  import Trash from '@lucide/svelte/icons/trash';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import BaseForm from '@profidev/pleiades/components/form/base-form.svelte';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { goto } from '$app/navigation';
  import { z } from 'zod';
  import {
    deleteServer,
    updateServer,
    type OptionEgg,
    type ServerInfo,
    type ServerOptions
  } from '$lib/client';
  import SaveBar from '$lib/components/form/SaveBar.svelte';
  import SectionErrorDot from '$lib/components/form/SectionErrorDot.svelte';
  import ApplyErrors from '$lib/components/form/ApplyErrors.svelte';
  import {
    eggStartup,
    eggVariables,
    findEgg,
    fromServerInfo,
    mapFieldErrors,
    optionsFromInfo,
    SECTION_DESCRIPTIONS,
    SECTION_KEYS,
    SECTION_TITLES,
    SECTIONS,
    serverSettings,
    stageForm,
    toUpdateBody,
    variablesEdited,
    type ServerForm
  } from '../schema.svelte';
  import { availableActions } from '../status';
  import StatusBadge from '../StatusBadge.svelte';
  import GeneralFields from '../GeneralFields.svelte';
  import StartupFields from '../StartupFields.svelte';
  import VariablesFields from '../VariablesFields.svelte';
  import ResourcesFields from '../ResourcesFields.svelte';
  import PermissionsFields from '../PermissionsFields.svelte';
  import StatusTab from './StatusTab.svelte';

  const { data } = $props();

  let server: ServerInfo | undefined = $state.raw();
  let options: ServerOptions | undefined = $state.raw();
  let saved: ServerForm | undefined = $state.raw();
  let form: BaseForm<typeof serverSettings> | undefined = $state();
  let saving = $state(false);
  let saveFailed = $state(false);
  let dirty = $state(false);
  let fieldErrors: Record<string, string> | undefined = $state.raw();
  let deleteOpen = $state(false);
  let isLoading = $state(false);
  let tab = $state('status');

  let admin = $derived(server?.access === 'admin');
  let readonly = $derived(
    server?.access !== 'admin' && server?.access !== 'write'
  );
  // Non-admins cannot load the option lists; the server's own references keep the selects filled.
  let fieldOptions = $derived(
    options ?? (server ? optionsFromInfo(server) : undefined)
  );
  let canDelete = $derived(
    admin && !!server && availableActions(server.status).delete
  );
  let sections = $derived(SECTIONS.filter((s) => s !== 'permissions' || admin));

  $effect(() => {
    data.serverRes.then((res) => {
      if (!res.data) {
        goto(
          res.response?.status === 404
            ? '/servers?error=not_found'
            : '/servers?error=other'
        );
        return;
      }
      server = res.data;
      // A reload (websocket update, status action) must not wipe unsaved edits.
      if (!dirty || !saved) saved = fromServerInfo(res.data);
    });
  });

  $effect(() => {
    data.options.then((o) => {
      options = o;
    });
  });

  const oneggchange = (egg: OptionEgg | undefined) => {
    const current = form?.getValue();
    if (!current) return;
    form?.setValue({
      ...current,
      ...eggStartup(egg),
      variables: eggVariables(egg)
    });
  };

  const onsubmit = async (values: ServerForm) => {
    const res = await updateServer({
      body: toUpdateBody(values, admin),
      path: { uuid: data.uuid }
    });

    if (!res.response?.ok) {
      const status = res.response?.status;
      if (status === 403) {
        saveFailed = false;
        return { error: "You don't have permission to change these fields." };
      }
      const mapped =
        status === 400 || status === 409
          ? mapFieldErrors(res.error, values)
          : undefined;
      if (!mapped) {
        saveFailed = true;
        return {
          error: "Couldn't save changes. Check your connection and retry."
        };
      }
      saveFailed = false;
      if (mapped.first) tab = mapped.first;
      const errors = Object.assign({}, ...Object.values(mapped.sections));
      // Applied after BaseForm finished handling this submit.
      setTimeout(() => (fieldErrors = errors));
      toast.error("Couldn't save server", {
        description:
          mapped.other.join(' ') ||
          (status === 409
            ? 'An allocation is already in use.'
            : 'Fix the highlighted fields.')
      });
      return { error: '' };
    }

    saveFailed = false;
    saved = values;
    if (server) server = { ...server, name: values.name };
    toast.success(`Server ${values.name} updated successfully`);
    // do not trigger form reset
    return { error: '' };
  };

  const deleteItemConfirm = async () => {
    isLoading = true;
    const res = await deleteServer({ path: { uuid: data.uuid } });
    isLoading = false;

    if (!res.response?.ok) {
      return { error: 'Failed to delete server' };
    }

    toast.success(`Server ${server?.name} deleted successfully`);
    setTimeout(() => {
      goto('/servers');
    });
  };
</script>

<div class="flex h-full max-h-screen min-h-0 w-full flex-col space-y-6 p-4">
  <div class="mt-1! mb-0 ml-7 flex flex-wrap items-center gap-2 md:m-0">
    <Button
      size="icon"
      variant="ghost"
      href="/servers"
      class="mr-2"
      aria-label="Back to servers"
    >
      <ArrowLeft class="size-5" />
    </Button>
    <h3 class="flex text-xl font-medium">
      Server:
      {#if !server}
        <Skeleton class="ml-2 h-7 w-20" />
      {:else}
        {server.name}
      {/if}
    </h3>
    {#if server}
      <StatusBadge status={server.status} />
    {/if}
    <Button
      class="ml-auto cursor-pointer"
      onclick={() => (deleteOpen = true)}
      variant="destructive"
      disabled={!canDelete}
    >
      <Trash />
      Delete
    </Button>
  </div>
  <Separator class="my-4" />
  {#if server && saved && fieldOptions}
    <BaseForm
      class="flex min-h-0 grow flex-col gap-4"
      schema={serverSettings}
      initialValue={saved}
      {onsubmit}
      bind:this={form}
      bind:isLoading={saving}
    >
      {#snippet children({ props })}
        {@const values = fromStore(props.formData.form)}
        {@const disabled = props.disabled || readonly}
        <ApplyErrors formData={props.formData} errors={fieldErrors} />
        <div class="mx-auto flex min-h-0 w-full max-w-4xl grow flex-col gap-4">
          {#if readonly}
            <Alert.Root>
              <Info />
              <Alert.Title>You have view-only access</Alert.Title>
              <Alert.Description>
                Fields are read-only. Ask an administrator for write access to
                make changes.
              </Alert.Description>
            </Alert.Root>
          {:else if !admin}
            <Alert.Root>
              <Info />
              <Alert.Title>Some settings need an administrator</Alert.Title>
              <Alert.Description>
                You can change the name, startup command and variables. Owner,
                egg, allocations, image and resources are read-only.
              </Alert.Description>
            </Alert.Root>
          {/if}
          <Tabs.Root bind:value={tab} class="flex min-h-0 grow flex-col gap-4">
            <Tabs.List
              class="w-full shrink-0 overflow-x-auto overflow-y-hidden"
            >
              <Tabs.Trigger value="status" class="cursor-pointer">
                Status
              </Tabs.Trigger>
              {#each sections as section (section)}
                <Tabs.Trigger value={section} class="cursor-pointer">
                  {SECTION_TITLES[section]}
                  <SectionErrorDot
                    formData={props.formData}
                    keys={SECTION_KEYS[section]}
                  />
                </Tabs.Trigger>
              {/each}
            </Tabs.List>
            <Tabs.Content
              value="status"
              class="min-h-0 overflow-y-auto data-[state=inactive]:hidden"
            >
              {#if server}
                <StatusTab
                  {server}
                  nodes={options?.nodes ?? []}
                  ondelete={() => (deleteOpen = true)}
                />
              {/if}
            </Tabs.Content>
            {#each sections as section (section)}
              <Tabs.Content
                value={section}
                class="flex min-h-0 flex-col data-[state=inactive]:hidden"
              >
                <Card.Root class="min-h-0">
                  <Card.Header>
                    <Card.Title>{SECTION_TITLES[section]}</Card.Title>
                    <Card.Description>
                      {SECTION_DESCRIPTIONS[section]}
                    </Card.Description>
                  </Card.Header>
                  <Card.Content class="min-h-0 overflow-y-auto">
                    <div class="grid gap-4">
                      {#if section === 'general'}
                        <GeneralFields
                          formData={stageForm(props.formData)}
                          {disabled}
                          options={fieldOptions}
                          mode="edit"
                          locked={!admin}
                          variablesEdited={variablesEdited(
                            values.current.variables
                          )}
                          {oneggchange}
                        />
                      {:else if section === 'startup'}
                        <StartupFields
                          formData={stageForm(props.formData)}
                          {disabled}
                          egg={findEgg(options, values.current.egg[0])}
                          mode="edit"
                          imageDisabled={!admin}
                        />
                      {:else if section === 'variables'}
                        <VariablesFields
                          formData={stageForm(props.formData)}
                          {disabled}
                        />
                      {:else if section === 'resources'}
                        <ResourcesFields
                          formData={stageForm(props.formData)}
                          disabled={disabled || !admin}
                        />
                      {:else}
                        <PermissionsFields
                          formData={stageForm(props.formData)}
                          {disabled}
                          options={fieldOptions}
                        />
                      {/if}
                    </div>
                  </Card.Content>
                </Card.Root>
              </Tabs.Content>
            {/each}
          </Tabs.Root>
        </div>
        <!-- Hidden, not unmounted: remounting would reset unsaved edits. -->
        <div class={tab === 'status' ? 'hidden' : 'contents'}>
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
  {:else}
    <div class="mx-auto grid w-full max-w-4xl gap-4" aria-busy="true">
      <Skeleton class="h-9 w-96 max-w-full" />
      <Skeleton class="h-32 w-full" />
      <Skeleton class="h-64 w-full" />
    </div>
  {/if}
</div>

<FormDialog
  title={`Delete server ${server?.name}?`}
  description="This cannot be undone. The server's files are removed and its allocations freed."
  confirm="Delete"
  confirmVariant="destructive"
  onsubmit={deleteItemConfirm}
  bind:open={deleteOpen}
  bind:isLoading
  schema={z.object({})}
/>
