<script lang="ts">
  import type { Stage } from '$lib/components/form/types.svelte';
  import MultiStepForm from '$lib/components/form/MultiStepForm.svelte';
  import AdminUser from './AdminUser.svelte';
  import DatabaseSetup from './DatabaseSetup.svelte';
  import type { PageData } from './$types';
  import CheckIcon from '@lucide/svelte/icons/check';
  import type { FormValue } from 'positron-components/components/form/types';
  import type { adminUser } from './schema.svelte';
  import { performSetup } from '$lib/backend/setup.svelte';
  import { RequestError } from 'positron-components/backend';
  import { goto } from '$app/navigation';
  import { connectWebsocket } from '$lib/backend/updater.svelte';

  interface Props {
    data: PageData;
  }

  let { data }: Props = $props();

  let stages: Stage<{ db_backend: string }>[] = [
    {
      title: 'Database Setup',
      content: DatabaseSetup,
      data: {}
    },
    {
      title: 'Admin User',
      content: AdminUser,
      data: {}
    }
  ];

  const submit = async (rawData: object) => {
    let data: FormValue<typeof adminUser> = rawData as any;

    let ret = await performSetup({
      admin_email: data.email,
      admin_password: data.password,
      admin_username: data.username
    });

    if (ret) {
      if (ret === RequestError.Conflict) {
        return { error: 'The setup was already completed.' };
      } else if (ret === RequestError.InternalServerError) {
        return { error: 'The server failed to find the admin group.' };
      } else {
        return { error: 'An unknown error occurred.' };
      }
    } else {
      setTimeout(() => {
        connectWebsocket();
        goto('/');
      });
    }
  };
</script>

<MultiStepForm
  {stages}
  onsubmit={submit}
  {data}
  submitLabel="Finish"
  submitIcon={CheckIcon}
/>
