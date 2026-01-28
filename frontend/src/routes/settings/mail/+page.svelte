<script lang="ts">
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import { mailSettings, reformat, unReformat } from './schema.svelte';
  import type { FormValue } from 'positron-components/components/form/types';
  import { Button } from 'positron-components/components/ui/button';
  import { Spinner } from 'positron-components/components/ui/spinner';
  import Save from '@lucide/svelte/icons/save';
  import { saveMailSettings } from '$lib/backend/settings.svelte';
  import { toast } from 'positron-components/components/util/general';
  import { Permission } from '$lib/permissions.svelte';
  import FormSwitch from 'positron-components/components/form/form-switch.svelte';
  import FormInput from 'positron-components/components/form/form-input.svelte';
  import FormInputPassword from '$lib/components/form/FormInputPassword.svelte';

  let { data } = $props();

  // svelte-ignore state_referenced_locally
  let smtpEnabled = $state(!!data.settings?.smtp);
  $effect(() => {
    smtpEnabled = !!data.settings?.smtp;
  });
  let readonly = $derived(
    !data.user?.permissions.includes(Permission.SETTINGS_EDIT)
  );

  const onsubmit = async (form: FormValue<typeof mailSettings>) => {
    let data = reformat(form);
    let ret = await saveMailSettings(data);

    if (ret) {
      toast.error('Failed to save mail settings');
    } else {
      toast.success('Mail settings saved successfully');
    }
    // do not trigger form reset
    return { error: '' };
  };
</script>

<h4 class="mb-2">Mail Settings</h4>
<BaseForm
  schema={mailSettings}
  {onsubmit}
  initialValue={unReformat(data.settings ?? {})}
>
  {#snippet children({ props })}
    <div class="grid grid-cols-1 gap-8 lg:grid-cols-2">
      <div class="flex flex-col gap-1">
        <FormSwitch
          {...props}
          key="smtp_enabled"
          label="Enable SMTP"
          onCheckedChange={(v) => (smtpEnabled = v)}
          disabled={readonly}
        />
        {#if smtpEnabled}
          <FormInput
            {...props}
            label="SMTP Host"
            key="smtp_host"
            placeholder="mail.example.com"
            {readonly}
          />
          <FormInput
            {...props}
            label="SMTP Port"
            key="smtp_port"
            placeholder="587"
            type="number"
            {readonly}
          />
          <FormInput
            {...props}
            label="SMTP Username"
            key="smtp_user"
            placeholder="user@example.com"
            {readonly}
          />
          <FormInputPassword
            {...props}
            label="SMTP Password"
            key="smtp_password"
            placeholder="Password"
            {readonly}
          />
          <FormInput
            {...props}
            label="From Address"
            key="smtp_from_address"
            placeholder="no-reply@example.com"
            {readonly}
          />
          <FormInput
            {...props}
            label="From Name"
            key="smtp_from_name"
            placeholder="Example App"
            {readonly}
          />
          <FormSwitch
            {...props}
            key="use_tls"
            label="Use TLS"
            disabled={readonly}
          />
        {/if}
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
