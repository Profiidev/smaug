<script lang="ts">
  import BaseForm from '@profidev/pleiades/components/form/base-form.svelte';
  import type { FormValue } from '@profidev/pleiades/components/form/types';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Spinner } from '@profidev/pleiades/components/ui/spinner';
  import Save from '@lucide/svelte/icons/save';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { Permission } from '$lib/permissions.svelte';
  import {
    advancedSettings,
    generalSettings,
    reformatData,
    undoReformatData
  } from '../../create/schema.svelte.js';
  import GeneralSettingsFields from '../../create/GeneralSettingsFields.svelte';
  import AdvancedSettingsFields from '../../create/AdvancedSettingsFields.svelte';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { updateNode, type NodeInfo, type UserInfo } from '$lib/client';

  const schema = generalSettings.extend(advancedSettings.shape);

  let { data } = $props();

  let node: NodeInfo | undefined = $state();
  let user: UserInfo | undefined = $state();
  let form: BaseForm<typeof schema> | undefined = $state();
  let readonly = $derived(!user?.permissions.includes(Permission.NODE_EDIT));

  $effect(() => {
    data.nodeRes.then((res) => {
      if (!res.data) return;
      node = res.data;
      form?.setValue(undoReformatData(node));
    });
  });

  $effect(() => {
    data.user.then((d) => {
      user = d;
    });
  });

  const onsubmit = async (form: FormValue<typeof schema>) => {
    let node = reformatData(form);
    let res = await updateNode({
      body: node,
      path: {
        uuid: data.uuid
      }
    });

    if (res.response?.status !== 200) {
      if (res.response?.status === 409) {
        return {
          error: 'This node name is already in use',
          field: 'name'
        } as const;
      } else if (res.response?.status === 400) {
        return { error: 'Invalid node address', field: 'address' } as const;
      } else {
        return { error: 'Failed to update node' };
      }
    } else {
      toast.success(`Node ${node.name} updated successfully`);
      // do not trigger form reset
      return { error: '' };
    }
  };
</script>

<h4 class="mb-2">Settings</h4>
<BaseForm {schema} {onsubmit} bind:this={form}>
  {#snippet children({ props })}
    <div class="grid grid-cols-1 gap-4 lg:grid-cols-[1fr_auto_1fr]">
      <div>
        {/* @ts-ignore */ null}
        <GeneralSettingsFields {...props} {readonly} secure={node?.secure} />
      </div>
      <Separator orientation="vertical" class="hidden lg:block" />
      <div>
        {/* @ts-ignore */ null}
        <AdvancedSettingsFields
          {...props}
          {readonly}
          cpuUnlimit={!node?.cpu_limit}
          memoryUnlimit={!node?.memory_limit_mb}
          storageUnlimit={!node?.disk_limit_mb}
        />
      </div>
    </div>
  {/snippet}
  {#snippet footer({ isLoading }: { isLoading: boolean })}
    <Button class="ml-auto cursor-pointer" type="submit" disabled={isLoading}>
      {#if isLoading}
        <Spinner />
      {:else}
        <Save />
      {/if}
      Save Changes</Button
    >
  {/snippet}
</BaseForm>
