<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { toast } from '@profidev/pleiades/components/util/general';
  import type { Stage } from '@profidev/pleiades/components/form/types';
  import MultistepForm from '@profidev/pleiades/components/form/multistep-form.svelte';
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import Info from '@lucide/svelte/icons/info';
  import General from '../General.svelte';
  import Variables from '../Variables.svelte';
  import InstallScript from '../InstallScript.svelte';
  import Runtime from '../Runtime.svelte';
  import Config from '../Config.svelte';
  import ImportButton from '../ImportButton.svelte';
  import { createEgg } from '$lib/client';
  import {
    apiErrorMessage,
    STAGE_KEYS,
    toEggBody,
    type EggForm,
    type EggImport
  } from '../schema.svelte';

  const STAGES = [
    { content: General, title: 'General' },
    { content: Variables, title: 'Variables' },
    { content: InstallScript, title: 'Install script' },
    { content: Runtime, title: 'Runtime' },
    { content: Config, title: 'Config' }
  ];

  const buildStages = (values?: EggForm): Stage[] =>
    STAGES.map((s, i) => ({
      ...s,
      data: values
        ? Object.fromEntries(
            STAGE_KEYS[i].map((k) => [k, values[k as keyof EggForm]])
          )
        : {}
    }));

  // An import started on the list page arrives through history state.
  let imported: EggImport | undefined = $state.raw(page.state.eggImport);
  // MultistepForm re-applies each stage's `data` when `stages` is replaced.
  let stages = $derived(buildStages(imported?.form));

  const onimport = (egg: EggImport, fileName: string) => {
    imported = egg;
    toast.success(`Imported ${fileName}`);
  };

  const submit = async (rawData: object) => {
    const res = await createEgg({ body: toEggBody(rawData as EggForm) });

    if (!res.data) {
      return { error: apiErrorMessage(res.error, 'Error creating egg.') };
    }

    const { uuid } = res.data;
    toast.success('Egg created successfully.');
    setTimeout(() => {
      goto(`/eggs/${uuid}/general`);
    });
  };
</script>

<!-- Header and card stack as one centered group; the min-h-0 flex chain caps the card so only the stage fields scroll. -->
<div class="flex h-full max-h-screen min-h-0 flex-col justify-center p-4">
  <div class="mx-auto flex min-h-0 w-full max-w-4xl flex-col gap-4">
    <div class="flex flex-wrap items-center gap-2">
      <div class="ml-7 flex items-center md:m-0">
        <Button
          class="cursor-pointer"
          size="icon"
          variant="ghost"
          href="/eggs"
          aria-label="Back to eggs"
        >
          <ArrowLeft class="size-5" />
        </Button>
        <h3 class="ml-2 text-xl font-medium">Create egg</h3>
      </div>
      <div class="ml-auto">
        <ImportButton {onimport} />
      </div>
    </div>
    {#if imported}
      <Alert.Root>
        <Info />
        <Alert.Title>Imported from {imported.sourceVersion}</Alert.Title>
        <Alert.Description>
          Upgraded to PLCN_v3. Review every step before creating the egg.
        </Alert.Description>
      </Alert.Root>
    {/if}
    <!-- Undo MultistepForm's centering wrapper (h-full, p-4) so the card sits right under the header. -->
    <div
      class="flex min-h-0 flex-col [&>div]:h-auto [&>div]:min-h-0 [&>div]:flex-col [&>div]:items-stretch [&>div]:p-0"
    >
      <MultistepForm
        {stages}
        class="min-h-0 w-full [&>[data-slot=card-content]]:flex [&>[data-slot=card-content]]:min-h-0 [&>[data-slot=card-content]]:flex-col"
        onsubmit={submit}
        cancelHref="/eggs"
      />
    </div>
  </div>
</div>
