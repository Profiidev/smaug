<script lang="ts">
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import type { FormValue } from 'positron-components/components/form/types';
  import { Button } from 'positron-components/components/ui/button';
  import { Spinner } from 'positron-components/components/ui/spinner';
  import Save from '@lucide/svelte/icons/save';
  import { toast } from 'positron-components/components/util/general';
  import { Permission } from '$lib/permissions.svelte';
  import {
    advancedSettings,
    generalSettings,
    reformatData,
    undoReformatData
  } from '../../create/schema.svelte.js';
  import GeneralSettingsFields from '../../create/GeneralSettingsFields.svelte';
  import AdvancedSettingsFields from '../../create/AdvancedSettingsFields.svelte';
  import { updateNode } from '$lib/backend/node.svelte.js';
  import { RequestError } from 'positron-components/backend';
  import { Separator } from 'positron-components/components/ui/separator';

  const schema = generalSettings.extend(advancedSettings.shape);

  let { data } = $props();

  let readonly = $derived(
    !data.user?.permissions.includes(Permission.NODE_EDIT)
  );

  const onsubmit = async (form: FormValue<typeof schema>) => {
    let node = reformatData(form);
    let res = await updateNode(data.node.id, node);

    if (res) {
      if (res === RequestError.Conflict) {
        return { error: 'This node name is already in use', field: 'name' };
      } else if (res === RequestError.BadRequest) {
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

<h4 class="mb-2">Settings</h4>
<BaseForm {schema} {onsubmit} initialValue={undoReformatData(data.node)}>
  {#snippet children({ props })}
    <div class="grid grid-cols-1 gap-4 lg:grid-cols-[1fr_auto_1fr]">
      <div>
        {/* @ts-ignore */ null}
        <GeneralSettingsFields
          {...props}
          {readonly}
          secure={data.node.secure}
        />
      </div>
      <Separator orientation="vertical" class="hidden lg:block" />
      <div>
        {/* @ts-ignore */ null}
        <AdvancedSettingsFields
          {...props}
          {readonly}
          cpuUnlimit={!data.node.cpu_limit}
          memoryUnlimit={!data.node.memory_limit_mb}
          storageUnlimit={!data.node.disk_limit_mb}
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
