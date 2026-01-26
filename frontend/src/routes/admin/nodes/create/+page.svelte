<script lang="ts">
  import { reformatData } from './schema.svelte';
  import { goto } from '$app/navigation';
  import GeneralSettings from './GeneralSettings.svelte';
  import AdvancedSettings from './AdvancedSettings.svelte';
  import { createNode } from '$lib/backend/node.svelte';
  import { RequestError } from 'positron-components/backend';
  import { toast } from 'positron-components/components/util/general';
  import type { Stage } from '$lib/components/form/types.svelte';
  import MultiStepForm from '$lib/components/form/MultiStepForm.svelte';

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
    let res = await createNode(data);

    if (typeof res === 'string') {
      if (res === RequestError.Conflict) {
        return { error: 'A node with this name already exists.' };
      } else if (res === RequestError.BadRequest) {
        return { error: 'Invalid node address.' };
      } else {
        return { error: 'Error creating deployment.' };
      }
    } else {
      toast.success('Node created successfully.');
      setTimeout(() => {
        goto(`/admin/nodes/${res.uuid}`);
      });
    }
  };
</script>

<MultiStepForm {stages} onsubmit={submit} />
