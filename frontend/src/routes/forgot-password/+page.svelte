<script lang="ts">
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import FormInput from 'positron-components/components/form/form-input.svelte';
  import * as Card from 'positron-components/components/ui/card';
  import { forgotPassword } from './schema.svelte';
  import type { FormValue } from 'positron-components/components/form/types';
  import { sendResetLink } from '$lib/backend/mail.svelte';
  import { toast } from 'positron-components/components/util/general';

  const onsubmit = async (data: FormValue<typeof forgotPassword>) => {
    let ret = await sendResetLink(data);

    if (ret) {
      return { error: 'Failed to send reset link.' };
    } else {
      toast.success('Reset link sent to your email address.');
    }
  };
</script>

<div class="flex h-screen w-full items-center justify-center px-4">
  <Card.Root class="mx-auto w-full max-w-sm">
    <Card.Header>
      <Card.Title class="text-2xl">Forgot Password</Card.Title>
      <Card.Description
        >Enter your email address below to reset your password</Card.Description
      >
    </Card.Header>
    <Card.Content>
      <BaseForm schema={forgotPassword} {onsubmit}>
        {#snippet children({ props })}
          <FormInput
            {...props}
            label="Email"
            type="email"
            placeholder="mail@example.com"
            key="email"
          />
        {/snippet}
        {#snippet footer({ defaultBtn })}
          {@render defaultBtn({ content: 'Reset Password' })}
        {/snippet}
      </BaseForm>
    </Card.Content>
  </Card.Root>
</div>
