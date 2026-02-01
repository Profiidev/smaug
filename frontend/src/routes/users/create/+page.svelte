<script lang="ts">
  import { goto } from '$app/navigation';
  import { RequestError } from 'positron-components/backend';
  import { toast } from 'positron-components/components/util/general';
  import type { Stage } from '$lib/components/form/types.svelte';
  import MultiStepForm from '$lib/components/form/MultiStepForm.svelte';
  import Information from './Information.svelte';
  import { createUser } from '$lib/backend/user.svelte';

  let { data } = $props();

  let stages: Stage<{
    mailActive: boolean;
  }>[] = [
    {
      title: 'Information',
      content: Information,
      data: {}
    }
  ];

  const submit = async (rawData: object) => {
    let anyData = rawData as any;
    if (
      !data.mailActive &&
      (!anyData.password || anyData.password.length < 6)
    ) {
      return {
        error: 'Password must be at least 6 characters long.',
        path: 'password'
      };
    }

    let res = await createUser(anyData);

    if (typeof res === 'string') {
      if (res === RequestError.Conflict) {
        return {
          error: 'A user with this email already exists.',
          path: 'email'
        };
      } else {
        return { error: 'Error creating user.' };
      }
    } else {
      toast.success('User created successfully.');
      setTimeout(() => {
        goto(`/users/${res.uuid}`);
      });
    }
  };
</script>

<MultiStepForm
  {stages}
  onsubmit={submit}
  data={{ mailActive: data.mailActive }}
  cancelHref="/users"
/>
