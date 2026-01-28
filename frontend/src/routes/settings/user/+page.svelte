<script lang="ts">
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import { reformat, unReformat, userSettings } from './schema.svelte';
  import type { FormValue } from 'positron-components/components/form/types';
  import { Button } from 'positron-components/components/ui/button';
  import { Spinner } from 'positron-components/components/ui/spinner';
  import Save from '@lucide/svelte/icons/save';
  import { saveUserSettings } from '$lib/backend/settings.svelte';
  import { toast } from 'positron-components/components/util/general';
  import FormInputTooltip from '$lib/components/form/FormInputTooltip.svelte';
  import FormSwitch from 'positron-components/components/form/form-switch.svelte';
  import FormInput from 'positron-components/components/form/form-input.svelte';
  import { RequestError } from 'positron-components/backend';
  import { Label } from 'positron-components/components/ui/label';
  import { Input } from 'positron-components/components/ui/input';
  import FormInputPassword from '$lib/components/form/FormInputPassword.svelte';

  let { data } = $props();

  // svelte-ignore state_referenced_locally
  let oidcEnabled = $state(!!data.settings?.oidc);
  $effect(() => {
    oidcEnabled = !!data.settings?.oidc;
  });

  const onsubmit = async (form: FormValue<typeof userSettings>) => {
    let data = reformat(form);
    let ret = await saveUserSettings(data);

    if (ret) {
      if (ret === RequestError.NotAcceptable) {
        return {
          path: 'oidc_issuer',
          error:
            'Invalid OIDC configuration URL. Check the server logs for more information.'
        };
      }
      toast.error('Failed to save user settings');
    } else {
      toast.success('User settings saved successfully');
    }
    // do not trigger form reset
    return { error: '' };
  };
</script>

<h4 class="mb-2">User Settings</h4>
<BaseForm
  schema={userSettings}
  {onsubmit}
  initialValue={unReformat(
    data.settings ?? {
      sso_create_user: true,
      sso_instant_redirect: true
    }
  )}
>
  {#snippet children({ props })}
    <div class="grid grid-cols-1 gap-8 lg:grid-cols-2">
      <div>
        <FormSwitch
          {...props}
          key="oidc_enabled"
          label="Enable SSO via OpenID Connect"
          onCheckedChange={(v) => (oidcEnabled = v)}
        />
        {#if oidcEnabled}
          <FormInputTooltip
            {...props}
            label="OpenID Connect Config URL"
            key="oidc_issuer"
            tooltip="The URL where the OpenID Connect configuration can be found. Without .well-known/openid-configuration at the end."
            placeholder="https://accounts.example.com"
          />
          <FormInput
            {...props}
            label="OpenID Connect Client ID"
            key="oidc_client_id"
            placeholder="your-client-id"
          />
          <FormInputPassword
            {...props}
            label="OpenID Connect Client Secret"
            key="oidc_client_secret"
            placeholder="your-client-secret"
          />
          <FormInput
            {...props}
            label="OpenID Connect Scopes (space separated)"
            key="oidc_scopes"
            placeholder="openid profile email"
          />
          <Label for="callback-url">Callback URL</Label>
          <Input
            id="callback-url"
            value={`${data.generalSettings?.site_url}/api/auth/oidc/callback`}
            readonly
            class="mt-2"
          />
        {/if}
      </div>
      <div>
        <FormSwitch
          {...props}
          key="sso_create_user"
          label="Create missing users on SSO login"
        />
        <FormSwitch
          {...props}
          key="sso_instant_redirect"
          label="Instantly redirect to SSO provider when accessing the login page"
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
