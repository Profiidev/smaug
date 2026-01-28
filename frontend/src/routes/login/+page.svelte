<script lang="ts">
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import FormInput from 'positron-components/components/form/form-input.svelte';
  import { Button } from 'positron-components/components/ui/button';
  import * as Card from 'positron-components/components/ui/card';
  import { FieldSeparator } from 'positron-components/components/ui/field';
  import { login } from './schema.svelte';
  import type { FormValue } from 'positron-components/components/form/types';
  import { passwordLogin } from '$lib/backend/auth.svelte';
  import { RequestError } from 'positron-components/backend';
  import { goto } from '$app/navigation';
  import { connectWebsocket } from '$lib/backend/updater.svelte';
  import { toast } from 'positron-components/components/util/general';
  import { SSOType } from '$lib/backend/sso.svelte';

  let { data } = $props();

  $effect(() => {
    if (data.error) {
      let error = '';
      switch (data.error) {
        case 'missing_code':
          error = 'SSO login failed: Missing authorization code.';
          break;
        case 'oidc_not_configured':
          error = 'SSO login failed: OIDC is not configured.';
          break;
        case 'user_not_found':
          error = 'User not found.';
          break;
        default:
          error = `SSO login failed: ${data.error}`;
      }

      toast.error(error);

      const url = new URL(window.location.href);
      url.searchParams.delete('error');
      window.history.replaceState({}, '', url);
    }
  });

  const onsubmit = async (data: FormValue<typeof login>) => {
    let ret = await passwordLogin(data.email, data.password);

    if (ret === RequestError.Unauthorized) {
      return { error: 'Invalid email or password.' };
    } else if (ret) {
      return { error: 'Login failed. Please try again later.' };
    } else {
      setTimeout(() => {
        connectWebsocket();
        goto('/');
      });
    }
  };
</script>

<div class="flex h-screen w-full items-center justify-center px-4">
  <Card.Root class="mx-auto w-full max-w-sm">
    <Card.Header>
      <Card.Title class="text-2xl">Login</Card.Title>
      <Card.Description
        >Enter your login details below to login</Card.Description
      >
    </Card.Header>
    <Card.Content>
      <BaseForm schema={login} {onsubmit}>
        {#snippet children({ props })}
          <FormInput
            {...props}
            label="Email"
            type="email"
            placeholder="mail@example.com"
            key="email"
          />
          <FormInput
            {...props}
            label="Password"
            type="password"
            placeholder="Your password"
            key="password"
          />
        {/snippet}
        {#snippet footer({ defaultBtn })}
          {@render defaultBtn({ content: 'Login' })}
        {/snippet}
      </BaseForm>
      {#if data.sso_config?.sso_type !== SSOType.None}
        <FieldSeparator
          class="*:data-[slot=field-separator-content]:bg-card my-4"
          >Or continue with</FieldSeparator
        >
        <Button
          variant="outline"
          class="w-full cursor-pointer"
          onclick={() => {
            if (!data.oidc_url) {
              toast.error('Failed to get OIDC URL.');
              return;
            }
            window.location.href = data.oidc_url;
          }}>OIDC Provider</Button
        >
      {/if}
    </Card.Content>
  </Card.Root>
</div>
