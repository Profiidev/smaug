<script lang="ts">
  import { Separator } from 'positron-components/components/ui/separator';
  import { Button } from 'positron-components/components/ui/button';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import Trash from '@lucide/svelte/icons/trash';
  import { Permission } from '$lib/permissions.svelte';
  import FormDialog from 'positron-components/components/form/form-dialog.svelte';
  import { z } from 'zod';
  import { toast } from 'positron-components/components/util/general';
  import { goto } from '$app/navigation';
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import { formatData, userSettings, reformatData } from './schema.svelte.js';
  import type { FormValue } from 'positron-components/components/form/types';
  import { RequestError } from 'positron-components/backend';
  import FormInput from 'positron-components/components/form/form-input.svelte';
  import Save from '@lucide/svelte/icons/save';
  import { Spinner } from 'positron-components/components/ui/spinner';
  import FormSelect from 'positron-components/components/form/form-select.svelte';
  import { deleteUser, editUser } from '$lib/backend/user.svelte.js';

  const { data } = $props();

  let deleteOpen = $state(false);
  let isLoading = $state(false);
  let readonly = $derived(
    !data.user?.permissions.includes(Permission.USER_EDIT)
  );

  const deleteItemConfirm = async () => {
    isLoading = true;
    let ret = await deleteUser(data.userInfo.uuid);
    isLoading = false;

    if (ret) {
      return { error: 'Failed to delete user' };
    } else {
      toast.success(`User ${data.userInfo.name} deleted successfully`);
      setTimeout(() => {
        goto('/users');
      });
    }
  };

  const onsubmit = async (form: FormValue<typeof userSettings>) => {
    let user = reformatData(form, data.userInfo.uuid);
    let res = await editUser(user);

    if (res) {
      if (res === RequestError.Conflict) {
        return { error: 'This user name is already in use', field: 'name' };
      } else {
        return { error: 'Failed to update user' };
      }
    } else {
      toast.success(`User ${data.userInfo.name} updated successfully`);
      // do not trigger form reset
      return { error: '' };
    }
  };
</script>

<div class="flex h-full w-full flex-col space-y-6 p-4">
  <div class="mt-1! mb-0 ml-7 flex items-center md:m-0">
    <Button size="icon" variant="ghost" href="/users" class="mr-2">
      <ArrowLeft class="size-5" />
    </Button>
    <h3 class="text-xl font-medium">User: {data.userInfo.name}</h3>
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
  <div
    class="flex grow flex-col space-y-4 lg:flex-row lg:space-y-0 lg:space-x-6"
  >
    <div class="flex-1">
      <h4 class="mb-2">Settings</h4>
      <BaseForm
        schema={userSettings}
        {onsubmit}
        initialValue={formatData(data.userInfo)}
      >
        {#snippet children({ props })}
          <div class="grid grid-cols-1 gap-4 lg:grid-cols-[1fr_auto_1fr]">
            <div>
              <FormInput
                {...props}
                key="name"
                label="User Name"
                placeholder="Enter user name"
                {readonly}
              />
              <FormSelect
                {...props}
                key="groups"
                label="Group Membership"
                data={data.groups?.map((group) => ({
                  label: group.name,
                  value: group.uuid
                })) || []}
              />
            </div>
          </div>
        {/snippet}
        {#snippet footer({ isLoading }: { isLoading: boolean })}
          <div class="mt-4 grid w-full grid-cols-1 lg:grid-cols-2">
            <Button
              class="ml-auto cursor-pointer"
              type="submit"
              disabled={isLoading}
            >
              {#if isLoading}
                <Spinner />
              {:else}
                <Save />
              {/if}
              Save Changes</Button
            >
          </div>
        {/snippet}
      </BaseForm>
    </div>
  </div>
</div>
<FormDialog
  title={`Delete User`}
  description={`Do you really want to delete the user ${data.userInfo.name}?`}
  confirm="Delete"
  confirmVariant="destructive"
  onsubmit={deleteItemConfirm}
  bind:open={deleteOpen}
  bind:isLoading
  schema={z.object({})}
/>
