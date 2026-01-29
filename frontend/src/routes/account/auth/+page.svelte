<script lang="ts">
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import { authSettings } from './schema.svelte';
  import type { FormValue } from 'positron-components/components/form/types';
  import { Button } from 'positron-components/components/ui/button';
  import { Spinner } from 'positron-components/components/ui/spinner';
  import Save from '@lucide/svelte/icons/save';
  import { toast } from 'positron-components/components/util/general';
  import { updatePassword } from '$lib/backend/user.svelte';
  import { RequestError } from 'positron-components/backend';
  import FormInputPassword from '$lib/components/form/FormInputPassword.svelte';

  let { data } = $props();

  const onsubmit = async (form: FormValue<typeof authSettings>) => {
    let ret = await updatePassword(form);

    if (ret) {
      if (ret === RequestError.Forbidden) {
        return { error: 'Old password is incorrect' };
      } else if (ret === RequestError.TooManyRequests) {
        return { error: 'Rate limit exceeded. Please try again later.' };
      } else {
        return { error: 'An unknown error occurred' };
      }
    } else {
      toast.success('Password updated successfully');
    }
    // do not trigger form reset
    return { error: '' };
  };
</script>

<h4 class="mb-2">Authentication</h4>
<BaseForm schema={authSettings} {onsubmit}>
  {#snippet children({ props })}
    <div class="grid grid-cols-1 gap-8 lg:grid-cols-2">
      <div class="flex flex-col gap-1">
        <FormInputPassword
          {...props}
          label="Old Password"
          key="old_password"
          placeholder="Enter your old password"
        />
        <FormInputPassword
          {...props}
          label="New Password"
          key="new_password"
          placeholder="Enter your new password"
        />
        <FormInputPassword
          {...props}
          label="Confirm New Password"
          key="new_password_confirm"
          placeholder="Enter your new password again"
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
        Update Password</Button
      >
    </div>
  {/snippet}
</BaseForm>
