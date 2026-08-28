<script lang="ts">
  import { goto } from '$app/navigation';
  import { toast } from '@profidev/pleiades/components/util/general';
  import type { Stage } from '@profidev/pleiades/components/form/types';
  import MultistepForm from '@profidev/pleiades/components/form/multistep-form.svelte';
  import General from '../General.svelte';
  import Variables from '../Variables.svelte';
  import InstallScript from '../InstallScript.svelte';
  import Runtime from '../Runtime.svelte';
  import Config from '../Config.svelte';
  import { reformatData, type EggJson } from '../schema.svelte';

  let stages: Stage[] = [
    { content: General, data: {}, title: 'General' },
    { content: Variables, data: {}, title: 'Variables' },
    { content: InstallScript, data: {}, title: 'Install Script' },
    { content: Runtime, data: {}, title: 'Runtime' },
    { content: Config, data: {}, title: 'Config' }
  ];

  const submit = async (rawData: object) => {
    const egg: EggJson = reformatData(rawData as any);

    // TODO: replace with createEgg({ body: egg }) from $lib/client once backend routes exist.
    void egg;

    toast.success('Egg created successfully.');
    setTimeout(() => {
      goto('/eggs');
    });
  };
</script>

<MultistepForm {stages} onsubmit={submit} cancelHref="/eggs" submitLabel="Create Egg" />
