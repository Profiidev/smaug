<script lang="ts">
  import type { Stage } from '$lib/components/form/types.svelte';
  import MultiStepForm from '$lib/components/form/MultiStepForm.svelte';
  import AdminUser from './AdminUser.svelte';
  import DatabaseSetup from './DatabaseSetup.svelte';
  import type { PageData } from './$types';

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

  const submit = async (rawData: object) => {};
</script>

<MultiStepForm {stages} onsubmit={submit} {data} />
