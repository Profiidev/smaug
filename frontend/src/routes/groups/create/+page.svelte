<script lang="ts">
  import { goto } from '$app/navigation';
  import { createGroup } from '$lib/backend/groups.svelte';
  import { RequestError } from 'positron-components/backend';
  import { toast } from 'positron-components/components/util/general';
  import type { Stage } from '$lib/components/form/types.svelte';
  import MultiStepForm from '$lib/components/form/MultiStepForm.svelte';
  import Information from './Information.svelte';

  let stages: Stage[] = [
    {
      title: 'Information',
      content: Information,
      data: {}
    }
  ];

  const submit = async (rawData: object) => {
    let res = await createGroup(rawData as any);

    if (typeof res === 'string') {
      if (res === RequestError.Conflict) {
        return {
          error: 'A group with this name already exists.',
          path: 'name'
        };
      } else {
        return { error: 'Error creating group.' };
      }
    } else {
      toast.success('Group created successfully.');
      setTimeout(() => {
        goto(`/groups/${res.uuid}`);
      });
    }
  };
</script>

<MultiStepForm {stages} onsubmit={submit} />
