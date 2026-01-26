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

  const onsubmit = async (data: FormValue<typeof login>) => {
    let ret = await passwordLogin(data.email, data.password);

    if (ret === RequestError.Unauthorized) {
      return { error: 'Invalid email or password.' };
    } else if (ret) {
      return { error: 'Login failed. Please try again later.' };
    } else {
      setTimeout(() => {
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
      <FieldSeparator class="*:data-[slot=field-separator-content]:bg-card my-4"
        >Or continue with</FieldSeparator
      >
      <Button variant="outline" class="w-full cursor-pointer"
        >OIDC Provider</Button
      >
    </Card.Content>
  </Card.Root>
</div>
