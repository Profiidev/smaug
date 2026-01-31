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
  import { deleteGroup, editGroup } from '$lib/backend/groups.svelte.js';
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import { formatData, groupSettings, reformatData } from './schema.svelte.js';
  import type { FormValue } from 'positron-components/components/form/types';
  import { RequestError } from 'positron-components/backend';
  import FormInput from 'positron-components/components/form/form-input.svelte';
  import Save from '@lucide/svelte/icons/save';
  import { Spinner } from 'positron-components/components/ui/spinner';
  import FormCheckbox from '$lib/components/form/FormCheckbox.svelte';
  import FormSelect from 'positron-components/components/form/form-select.svelte';

  const { data } = $props();

  let deleteOpen = $state(false);
  let isLoading = $state(false);
  let readonly = $derived(
    !data.user?.permissions.includes(Permission.GROUP_EDIT)
  );

  const deleteItemConfirm = async () => {
    isLoading = true;
    let ret = await deleteGroup({ uuid: data.group.id });
    isLoading = false;

    if (ret) {
      return { error: 'Failed to delete group' };
    } else {
      toast.success(`Group ${data.group.name} deleted successfully`);
      setTimeout(() => {
        goto('/groups');
      });
    }
  };

  const onsubmit = async (form: FormValue<typeof groupSettings>) => {
    let group = reformatData(form, data.group.id);
    let res = await editGroup(group);

    if (res) {
      if (res === RequestError.Conflict) {
        return { error: 'This group name is already in use', field: 'name' };
      } else {
        return { error: 'Failed to update group' };
      }
    } else {
      toast.success(`Group ${data.group.name} updated successfully`);
      // do not trigger form reset
      return { error: '' };
    }
  };
</script>

<div class="flex h-full w-full flex-col space-y-6 p-4">
  <div class="mt-1! mb-0 ml-7 flex items-center md:m-0">
    <Button size="icon" variant="ghost" href="/groups" class="mr-2">
      <ArrowLeft class="size-5" />
    </Button>
    <h3 class="text-xl font-medium">Group: {data.group.name}</h3>
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
        schema={groupSettings}
        {onsubmit}
        initialValue={formatData(data.group)}
      >
        {#snippet children({ props })}
          <div class="grid grid-cols-1 gap-4 lg:grid-cols-[1fr_auto_1fr]">
            <div>
              <FormInput
                {...props}
                key="name"
                label="Group Name"
                placeholder="Enter group name"
                {readonly}
              />
              <FormSelect
                {...props}
                key="users"
                label="Group Members"
                data={data.users?.map((user) => ({
                  label: user.name,
                  value: user.id
                })) || []}
              />
              <h5>Permissions</h5>
              <div class="ml-4">
                {#if data.user?.permissions.includes(Permission.NODE_VIEW) || data.user?.permissions.includes(Permission.NODE_EDIT)}
                  <h6>Nodes</h6>
                  <div class="ml-4">
                    {#if data.user?.permissions.includes(Permission.NODE_VIEW)}
                      <FormCheckbox
                        {...props}
                        key="node_view"
                        label="View Nodes"
                        disabled={readonly}
                      />
                    {/if}
                    {#if data.user?.permissions.includes(Permission.NODE_EDIT)}
                      <FormCheckbox
                        {...props}
                        key="node_edit"
                        label="Edit Nodes"
                        disabled={readonly}
                      />
                    {/if}
                  </div>
                {/if}
                {#if data.user?.permissions.includes(Permission.SETTINGS_VIEW) || data.user?.permissions.includes(Permission.SETTINGS_EDIT)}
                  <h6>Settings</h6>
                  <div class="ml-4">
                    {#if data.user?.permissions.includes(Permission.SETTINGS_VIEW)}
                      <FormCheckbox
                        {...props}
                        key="settings_view"
                        label="View Settings"
                        disabled={readonly}
                      />
                    {/if}
                    {#if data.user?.permissions.includes(Permission.SETTINGS_EDIT)}
                      <FormCheckbox
                        {...props}
                        key="settings_edit"
                        label="Edit Settings"
                        disabled={readonly}
                      />
                    {/if}
                  </div>
                {/if}
                {#if data.user?.permissions.includes(Permission.GROUP_VIEW) || data.user?.permissions.includes(Permission.GROUP_EDIT)}
                  <h6>Groups</h6>
                  <div class="ml-4">
                    {#if data.user?.permissions.includes(Permission.GROUP_VIEW)}
                      <FormCheckbox
                        {...props}
                        key="group_view"
                        label="View Groups"
                        disabled={readonly}
                      />
                    {/if}
                    {#if data.user?.permissions.includes(Permission.GROUP_EDIT)}
                      <FormCheckbox
                        {...props}
                        key="group_edit"
                        label="Edit Groups"
                        disabled={readonly}
                      />
                    {/if}
                  </div>
                {/if}
              </div>
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
  title={`Delete Group`}
  description={`Do you really want to delete the group ${data.group.name}?`}
  confirm="Delete"
  confirmVariant="destructive"
  onsubmit={deleteItemConfirm}
  bind:open={deleteOpen}
  bind:isLoading
  schema={z.object({})}
/>
