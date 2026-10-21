<script lang="ts">
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Card from '@profidev/pleiades/components/ui/card';
  import * as Tabs from '@profidev/pleiades/components/ui/tabs';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import Trash from '@lucide/svelte/icons/trash';
  import Download from '@lucide/svelte/icons/download';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import { Spinner } from '@profidev/pleiades/components/ui/spinner';
  import * as DropdownMenu from '@profidev/pleiades/components/ui/dropdown-menu';
  import { Permission } from '$lib/permissions.svelte';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import { z } from 'zod';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import BaseForm from '@profidev/pleiades/components/form/base-form.svelte';
  import { Skeleton } from '@profidev/pleiades/components/ui/skeleton';
  import { deleteEgg, editEgg, type UserInfo } from '$lib/client';
  import {
    apiErrorMessage,
    deleteError,
    eggSettings,
    formatData,
    reformatData,
    SECTION_DESCRIPTIONS,
    slugify,
    STAGE_KEYS,
    stageForm,
    toEggBody,
    type EggForm,
    type ExportVersion
  } from '../../schema.svelte';
  import GeneralFields from '../../GeneralFields.svelte';
  import VariablesFields from '../../VariablesFields.svelte';
  import InstallScriptFields from '../../InstallScriptFields.svelte';
  import RuntimeFields from '../../RuntimeFields.svelte';
  import ConfigFields from '../../ConfigFields.svelte';
  import SaveBar from '$lib/components/form/SaveBar.svelte';
  import SectionErrorDot from '$lib/components/form/SectionErrorDot.svelte';
  import UpdateDialog from '../../UpdateDialog.svelte';
  import { checkEggUpdate, type EggUpdate } from '../../update';

  const { data } = $props();

  let deleteOpen = $state(false);
  let isLoading = $state(false);
  let user: UserInfo | undefined = $state();
  let eggName: string | undefined = $state();
  let form: BaseForm<typeof eggSettings> | undefined = $state();
  let saved: EggForm | undefined = $state.raw();
  let saving = $state(false);
  let saveFailed = $state(false);
  let checking = $state(false);
  let update: EggUpdate | undefined = $state.raw();
  let updateOpen = $state(false);

  const sections = [
    {
      description: SECTION_DESCRIPTIONS.general,
      keys: STAGE_KEYS[0],
      title: 'General',
      value: 'general'
    },
    {
      description: SECTION_DESCRIPTIONS.variables,
      keys: STAGE_KEYS[1],
      title: 'Variables',
      value: 'variables'
    },
    {
      description: SECTION_DESCRIPTIONS.install,
      keys: STAGE_KEYS[2],
      title: 'Install script',
      value: 'install'
    },
    {
      description: SECTION_DESCRIPTIONS.runtime,
      keys: STAGE_KEYS[3],
      title: 'Runtime',
      value: 'runtime'
    },
    {
      description: SECTION_DESCRIPTIONS.config,
      keys: STAGE_KEYS[4],
      title: 'Config',
      value: 'config'
    }
  ];
  // The route matcher guarantees a valid tab; the component is reused across
  // tab URLs, so unsaved edits survive switching.
  let tab = $derived(page.params.tab ?? 'general');

  const setTab = (value: string) => {
    if (value !== tab) {
      goto(`/eggs/${data.uuid}/${value}`, { keepFocus: true, noScroll: true });
    }
  };

  let readonly = $derived(!user?.permissions.includes(Permission.EGG_EDIT));

  $effect(() => {
    data.eggRes.then((res) => {
      if (!res.data) {
        if (res.response?.status === 404) {
          goto('/eggs?error=not_found');
        } else {
          goto('/eggs?error=other');
        }
        return;
      }

      eggName = res.data.name;
      saved = formatData(res.data);
    });
  });

  $effect(() => {
    data.user.then((d) => {
      user = d;
    });
  });

  const deleteItemConfirm = async () => {
    isLoading = true;
    const res = await deleteEgg({ body: { uuid: data.uuid } });
    isLoading = false;

    if (!res.response?.ok) {
      return { error: deleteError(res.response?.status) };
    }

    toast.success(`Egg ${eggName} deleted successfully`);
    setTimeout(() => {
      goto('/eggs');
    });
  };

  const exportEgg = (version: ExportVersion) => {
    const values = form?.getValue();
    if (!values) return;
    const egg =
      version === 'PTDL_v2'
        ? reformatData(values, 'PTDL_v2')
        : reformatData(values, 'PLCN_v3', data.uuid);
    const url = URL.createObjectURL(
      new Blob([JSON.stringify(egg, null, 4)], { type: 'application/json' })
    );
    const a = document.createElement('a');
    a.href = url;
    a.download = `egg-${slugify(values.name)}.json`;
    a.click();
    URL.revokeObjectURL(url);
    toast.success('Export downloaded', {
      description: `${a.download} was saved.`
    });
  };

  const checkUpdate = async () => {
    if (!saved) return;
    if (!saved.updateUrl) {
      toast.info('No update URL', {
        description:
          'Add an Update URL in General to check this egg for updates.'
      });
      return;
    }

    checking = true;
    try {
      update = await checkEggUpdate(data.uuid, saved);
    } catch (e) {
      toast.error("Couldn't check for updates", {
        description: e instanceof Error ? e.message : undefined,
        action: { label: 'Retry', onClick: checkUpdate }
      });
      return;
    } finally {
      checking = false;
    }

    if (update.changed.length) {
      updateOpen = true;
    } else {
      toast.success(`${eggName} is up to date`, {
        description: 'The source has no changes.'
      });
    }
  };

  const onapplied = (egg: EggForm) => {
    saveFailed = false;
    saved = egg;
    eggName = egg.name;
  };

  const onsubmit = async (values: EggForm) => {
    const res = await editEgg({
      body: { ...toEggBody(values), uuid: data.uuid }
    });

    saveFailed = Boolean(res.error);
    if (res.error) {
      return { error: apiErrorMessage(res.error, 'Failed to update egg') };
    }

    saved = values;
    eggName = values.name;
    toast.success(`Egg ${eggName} updated successfully`);
    // do not trigger form reset
    return { error: '' };
  };
</script>

<div class="flex h-full max-h-screen min-h-0 w-full flex-col space-y-6 p-4">
  <div class="mt-1! mb-0 ml-7 flex flex-wrap items-center gap-y-2 md:m-0">
    <Button
      size="icon"
      variant="ghost"
      href="/eggs"
      class="mr-2 cursor-pointer"
      aria-label="Back to eggs"
    >
      <ArrowLeft class="size-5" />
    </Button>
    <h3 class="flex text-xl font-medium">
      Egg:
      {#if !eggName}
        <Skeleton class="ml-2 h-7 w-20" />
      {:else}
        {eggName}
      {/if}
    </h3>
    <Button
      variant="outline"
      class="ml-auto cursor-pointer"
      onclick={checkUpdate}
      disabled={checking || readonly || !saved}
    >
      {#if checking}
        <Spinner />
        Checking…
      {:else}
        <RefreshCw />
        Check for updates
      {/if}
    </Button>
    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <Button {...props} variant="outline" class="ml-2 cursor-pointer">
            <Download />
            Export
          </Button>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end" class="w-max whitespace-nowrap">
        <DropdownMenu.Item onSelect={() => exportEgg('PLCN_v3')}>
          Pelican (PLCN_v3)
        </DropdownMenu.Item>
        <DropdownMenu.Item onSelect={() => exportEgg('PTDL_v2')}>
          Pterodactyl (PTDL_v2)
        </DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
    <Button
      class="ml-2 cursor-pointer"
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
    schema={eggSettings}
    {onsubmit}
    bind:this={form}
    bind:isLoading={saving}
  >
    {#snippet children({ props })}
      <Tabs.Root
        bind:value={() => tab, setTab}
        class="mx-auto flex min-h-0 w-full max-w-4xl grow flex-col gap-4"
      >
        <Tabs.List class="w-full shrink-0 overflow-x-auto overflow-y-hidden">
          {#each sections as section (section.value)}
            <Tabs.Trigger value={section.value} class="cursor-pointer">
              {section.title}
              <SectionErrorDot formData={props.formData} keys={section.keys} />
            </Tabs.Trigger>
          {/each}
        </Tabs.List>
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
                    <GeneralFields
                      formData={stageForm(props.formData)}
                      disabled={props.disabled || readonly}
                    />
                  {:else if section.value === 'variables'}
                    <VariablesFields
                      formData={stageForm(props.formData)}
                      disabled={props.disabled || readonly}
                    />
                  {:else if section.value === 'install'}
                    <InstallScriptFields
                      formData={stageForm(props.formData)}
                      disabled={props.disabled || readonly}
                    />
                  {:else if section.value === 'runtime'}
                    <RuntimeFields
                      formData={stageForm(props.formData)}
                      disabled={props.disabled || readonly}
                    />
                  {:else}
                    <ConfigFields
                      formData={stageForm(props.formData)}
                      disabled={props.disabled || readonly}
                    />
                  {/if}
                </div>
              </Card.Content>
            </Card.Root>
          </Tabs.Content>
        {/each}
      </Tabs.Root>
      <SaveBar
        formData={props.formData}
        {saved}
        isLoading={saving}
        isError={saveFailed}
        {readonly}
      />
    {/snippet}
    {#snippet footer()}{/snippet}
  </BaseForm>
</div>
<UpdateDialog bind:open={updateOpen} {update} {onapplied} />

<FormDialog
  title={`Delete egg ${eggName}?`}
  description="This cannot be undone. The egg and its variables, scripts and config files are removed."
  confirm="Delete"
  confirmVariant="destructive"
  onsubmit={deleteItemConfirm}
  bind:open={deleteOpen}
  bind:isLoading
  schema={z.object({})}
/>
