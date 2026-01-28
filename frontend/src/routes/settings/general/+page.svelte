<script lang="ts">
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import { generalSettings } from './schema.svelte';
  import type { FormValue } from 'positron-components/components/form/types';
  import { Button } from 'positron-components/components/ui/button';
  import { Spinner } from 'positron-components/components/ui/spinner';
  import Save from '@lucide/svelte/icons/save';
  import { saveGeneralSettings } from '$lib/backend/settings.svelte';
  import { toast } from 'positron-components/components/util/general';
  import FormInputTooltip from '$lib/components/form/FormInputTooltip.svelte';
  import { Permission } from '$lib/permissions.svelte';

  let { data } = $props();

  let readonly = $derived(
    !data.user?.permissions.includes(Permission.SETTINGS_EDIT)
  );

  const onsubmit = async (form: FormValue<typeof generalSettings>) => {
    let ret = await saveGeneralSettings(form);

    if (ret) {
      toast.error('Failed to save general settings');
    } else {
      toast.success('General settings saved successfully');
    }
    // do not trigger form reset
    return { error: '' };
  };
</script>

<h4 class="mb-2">General Settings</h4>
<BaseForm schema={generalSettings} {onsubmit} initialValue={data.settings}>
  {#snippet children({ props })}
    <div class="grid grid-cols-1 gap-8 lg:grid-cols-2">
      <div class="flex flex-col gap-1">
        <FormInputTooltip
          {...props}
          label="Site URL"
          key="site_url"
          tooltip="The URL to reach Smaug from your browser."
          placeholder="https://smaug.example.com"
          {readonly}
        />
      </div>
    </div>
  {/snippet}
  {#snippet footer({ isLoading }: { isLoading: boolean })}
    <div class="mt-4 grid w-full grid-cols-1 lg:grid-cols-2">
      <Button class="ml-auto cursor-pointer" type="submit" disabled={isLoading}>
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
