<script lang="ts">
  import { goto } from '$app/navigation';
  import { toast } from '@profidev/pleiades/components/util/general';
  import type { Stage } from '@profidev/pleiades/components/form/types';
  import MultistepForm from '@profidev/pleiades/components/form/multistep-form.svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import { createNode } from '$lib/client';
  import GeneralSettings from '../GeneralSettings.svelte';
  import AdvancedSettings from '../AdvancedSettings.svelte';
  import { reformatData, type NodeForm } from '../schema.svelte';

  const stages: Stage[] = [
    { content: GeneralSettings, data: {}, title: 'General' },
    { content: AdvancedSettings, data: {}, title: 'Resources' }
  ];

  const submit = async (rawData: object) => {
    const res = await createNode({ body: reformatData(rawData as NodeForm) });

    if (!res.data) {
      if (res.response?.status === 409) {
        return { error: 'A node with this name already exists.' };
      } else if (res.response?.status === 400) {
        return { error: 'Invalid node address.' };
      }
      return { error: 'Error creating node.' };
    }

    const { token, uuid } = res.data;
    toast.success('Node created successfully.');
    // The token can not be fetched again; the Setup tab shows it once.
    sessionStorage.setItem('newNodeToken', token);
    setTimeout(() => {
      goto(`/nodes/${uuid}/setup`);
    });
  };
</script>

<!-- Header and card stack as one centered group; the min-h-0 flex chain caps the card so only the stage fields scroll. -->
<div class="flex h-full max-h-screen min-h-0 flex-col justify-center p-4">
  <div class="mx-auto flex min-h-0 w-full max-w-4xl flex-col gap-4">
    <div class="ml-7 flex items-center md:m-0">
      <Button
        class="cursor-pointer"
        size="icon"
        variant="ghost"
        href="/nodes"
        aria-label="Back to nodes"
      >
        <ArrowLeft class="size-5" />
      </Button>
      <h3 class="ml-2 text-xl font-medium">Create node</h3>
    </div>
    <!-- Undo MultistepForm's centering wrapper (h-full, p-4) so the card sits right under the header. -->
    <div
      class="flex min-h-0 flex-col [&>div]:h-auto [&>div]:min-h-0 [&>div]:flex-col [&>div]:items-stretch [&>div]:p-0"
    >
      <MultistepForm
        {stages}
        class="min-h-0 w-full [&>[data-slot=card-content]]:flex [&>[data-slot=card-content]]:min-h-0 [&>[data-slot=card-content]]:flex-col"
        onsubmit={submit}
        cancelHref="/nodes"
      />
    </div>
  </div>
</div>
