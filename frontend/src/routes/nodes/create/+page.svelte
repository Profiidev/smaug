<script lang="ts">
  import { reformatData } from './schema.svelte';
  import { goto } from '$app/navigation';
  import GeneralSettings from './GeneralSettings.svelte';
  import AdvancedSettings from './AdvancedSettings.svelte';
  import { toast } from '@profidev/pleiades/components/util/general';
  import type { Stage } from '@profidev/pleiades/components/form/types';
  import { createNode } from '$lib/client';
  import MultistepForm from '@profidev/pleiades/components/form/multistep-form.svelte';

  let stages: Stage[] = [
    {
      title: 'General Settings',
      content: GeneralSettings,
      data: {}
    },
    {
      title: 'Advanced Settings',
      content: AdvancedSettings,
      data: {}
    }
  ];

  const submit = async (rawData: object) => {
    let data = reformatData(rawData as any);
    let res = await createNode({
      body: data
    });

    if (res.response?.status !== 200 || !res.data) {
      if (res.response?.status === 409) {
        return { error: 'A node with this name already exists.' };
      } else if (res.response?.status === 400) {
        return { error: 'Invalid node address.' };
      } else {
        return { error: 'Error creating node.' };
      }
    } else {
      toast.success('Node created successfully.');
      setTimeout(() => {
        goto(`/nodes/${res.data.uuid}/setup`);
      });
    }
  };
</script>

<MultistepForm {stages} onsubmit={submit} cancelHref="/nodes" />
