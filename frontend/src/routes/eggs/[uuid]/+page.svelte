<script lang="ts">
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Card from '@profidev/pleiades/components/ui/card';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import Trash from '@lucide/svelte/icons/trash';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import Save from '@lucide/svelte/icons/save';
  import { Permission } from '$lib/permissions.svelte';
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import { z } from 'zod';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { goto } from '$app/navigation';
  import BaseForm from '@profidev/pleiades/components/form/base-form.svelte';
  import { Spinner } from '@profidev/pleiades/components/ui/spinner';
  import { Skeleton } from '@profidev/pleiades/components/ui/skeleton';
  import type { UserInfo } from '$lib/client';
  import type { FormValue } from '@profidev/pleiades/components/form/types';
  import { eggSettings, formatData, reformatData } from '../schema.svelte';
  import GeneralFields from '../GeneralFields.svelte';
  import VariablesFields from '../VariablesFields.svelte';
  import InstallScriptFields from '../InstallScriptFields.svelte';
  import RuntimeFields from '../RuntimeFields.svelte';
  import ConfigFields from '../ConfigFields.svelte';

  const { data } = $props();

  let deleteOpen = $state(false);
  let isLoading = $state(false);
  let user: UserInfo | undefined = $state();
  let eggName: string | undefined = $state();
  let form: BaseForm<typeof eggSettings> | undefined = $state();

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
      form?.setValue(formatData(res.data));
    });
  });

  $effect(() => {
    data.user.then((d) => {
      user = d;
    });
  });

  const deleteItemConfirm = async () => {
    isLoading = true;
    // TODO: replace with deleteEgg({ body: { uuid: data.uuid } }) from $lib/client
    // once backend egg routes exist.
    isLoading = false;

    toast.success(`Egg ${eggName} deleted successfully`);
    setTimeout(() => {
      goto('/eggs');
    });
  };

  const onsubmit = async (values: FormValue<typeof eggSettings>) => {
    const egg = reformatData(values);

    // TODO: replace with editEgg({ body: { ...egg, uuid: data.uuid } }) from $lib/client
    // once backend egg routes exist.
    void egg;

    toast.success(`Egg ${eggName} updated successfully`);
    // do not trigger form reset
    return { error: '' };
  };
</script>

<div class="flex h-full max-h-screen min-h-0 w-full flex-col space-y-6 p-4">
  <div class="mt-1! mb-0 ml-7 flex items-center md:m-0">
    <Button size="icon" variant="ghost" href="/eggs" class="mr-2">
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
    class="flex min-h-0 grow flex-col gap-4 overflow-y-auto"
    schema={eggSettings}
    {onsubmit}
    bind:this={form}
  >
    {#snippet children({ props })}
      <Card.Root>
        <Card.Header>
          <Card.Title>General</Card.Title>
        </Card.Header>
        <Card.Content class="flex flex-col gap-4">
          <GeneralFields {...(props as any)} disabled={props.disabled || readonly} />
        </Card.Content>
      </Card.Root>
      <Card.Root>
        <Card.Header>
          <Card.Title>Variables</Card.Title>
        </Card.Header>
        <Card.Content class="flex flex-col gap-4">
          <VariablesFields {...(props as any)} disabled={props.disabled || readonly} />
        </Card.Content>
      </Card.Root>
      <Card.Root>
        <Card.Header>
          <Card.Title>Install Script</Card.Title>
        </Card.Header>
        <Card.Content class="flex flex-col gap-4">
          <InstallScriptFields {...(props as any)} disabled={props.disabled || readonly} />
        </Card.Content>
      </Card.Root>
      <Card.Root>
        <Card.Header>
          <Card.Title>Runtime</Card.Title>
        </Card.Header>
        <Card.Content class="flex flex-col gap-4">
          <RuntimeFields {...(props as any)} disabled={props.disabled || readonly} />
        </Card.Content>
      </Card.Root>
      <Card.Root>
        <Card.Header>
          <Card.Title>Config</Card.Title>
        </Card.Header>
        <Card.Content class="flex flex-col gap-4">
          <ConfigFields {...(props as any)} disabled={props.disabled || readonly} />
        </Card.Content>
      </Card.Root>
    {/snippet}
    {#snippet footer({
      isLoading,
      isError
    }: {
      isLoading: boolean;
      isError: boolean;
    })}
      <div class="mt-4 flex justify-end">
        <Button
          class="cursor-pointer"
          type="submit"
          disabled={isLoading}
          variant={isError ? 'destructive' : undefined}
        >
          {#if isLoading}
            <Spinner />
          {:else if isError}
            <RotateCcw />
          {:else}
            <Save />
          {/if}
          {isError ? 'Retry' : 'Save Changes'}</Button
        >
      </div>
    {/snippet}
  </BaseForm>
</div>
<FormDialog
  title={`Delete Egg`}
  description={`Do you really want to delete the egg ${eggName}?`}
  confirm="Delete"
  confirmVariant="destructive"
  onsubmit={deleteItemConfirm}
  bind:open={deleteOpen}
  bind:isLoading
  schema={z.object({})}
/>
