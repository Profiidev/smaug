<script lang="ts">
  import { goto } from '$app/navigation';
  import { toast } from '@profidev/pleiades/components/util/general';
  import type { Stage } from '@profidev/pleiades/components/form/types';
  import MultistepForm from '@profidev/pleiades/components/form/multistep-form.svelte';
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Skeleton } from '@profidev/pleiades/components/ui/skeleton';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import Info from '@lucide/svelte/icons/info';
  import General from '../General.svelte';
  import Startup from '../Startup.svelte';
  import Variables from '../Variables.svelte';
  import Resources from '../Resources.svelte';
  import {
    createServer,
    type OptionEgg,
    type ServerOptions,
    type UserInfo
  } from '$lib/client';
  import {
    eggStartup,
    eggVariables,
    mapFieldErrors,
    SECTIONS,
    type MappedErrors,
    type Section,
    type ServerForm,
    type WizardData,
    toServerBody
  } from '../schema.svelte';

  const { data } = $props();

  let options: ServerOptions | undefined = $state.raw();
  let user: UserInfo | undefined = $state.raw();
  // Bumped to remount the wizard on the first stage with server errors.
  let attempt = $state(0);
  let initStep = $state(0);
  let pending: MappedErrors['sections'] = {};

  $effect(() => {
    Promise.all([data.options, data.user]).then(([o, u]) => {
      if (!o) {
        goto('/servers?error=options');
        return;
      }
      user = u;
      options = o;
    });
  });

  // Stage objects are plain (non-reactive): MultistepForm writes each stage's
  // value to `data` on Next / Previous and remounts a stage from it.
  const buildStages = (o: ServerOptions, u?: UserInfo): Stage<WizardData>[] => [
    {
      content: General,
      data: o.users.some((x) => x.uuid === u?.uuid) ? { owner: [u!.uuid] } : {},
      title: 'General'
    },
    { content: Startup, data: {}, title: 'Startup' },
    { content: Variables, data: {}, title: 'Variables' },
    { content: Resources, data: {}, title: 'Resources' }
  ];

  let stages = $derived(options ? buildStages(options, user) : undefined);

  const values = (): Partial<ServerForm> =>
    Object.assign({}, ...(stages ?? []).map((s) => s.data));

  let wizard: WizardData | undefined = $derived(
    options && {
      oneggchange: (egg: OptionEgg | undefined) => {
        if (!stages) return;
        stages[1].data = { ...stages[1].data, ...eggStartup(egg) };
        stages[2].data = { variables: eggVariables(egg) };
      },
      options,
      takeErrors: (section: Section) => {
        const errors = pending[section];
        delete pending[section];
        return errors;
      },
      values
    }
  );

  const submit = async (rawData: object) => {
    const form = rawData as ServerForm;
    const res = await createServer({ body: toServerBody(form) });

    if (!res.data) {
      const status = res.response?.status;
      const mapped =
        status === 400 || status === 409
          ? mapFieldErrors(res.error, form)
          : undefined;
      if (mapped?.first) {
        pending = mapped.sections;
        initStep = SECTIONS.indexOf(mapped.first);
        // Remount after BaseForm finished handling this submit.
        setTimeout(() => attempt++);
        return {
          error:
            status === 409
              ? "Couldn't create server: an allocation is already in use."
              : "Couldn't create server. Fix the highlighted fields."
        };
      }
      return {
        error: mapped?.other.join(' ') || 'Error creating server.'
      };
    }

    const { uuid } = res.data;
    toast.success('Server created successfully.');
    setTimeout(() => {
      goto(`/servers/${uuid}`);
    });
  };
</script>

<div class="flex h-full max-h-screen min-h-0 flex-col gap-4 p-4">
  <div class="mx-auto flex w-full max-w-4xl flex-wrap items-center gap-2">
    <div class="ml-7 flex items-center md:m-0">
      <Button
        size="icon"
        variant="ghost"
        href="/servers"
        aria-label="Back to servers"
      >
        <ArrowLeft class="size-5" />
      </Button>
      <h3 class="ml-2 text-xl font-medium">Create server</h3>
    </div>
  </div>
  {#if options && (!options.nodes.length || !options.eggs.length)}
    <Alert.Root class="mx-auto max-w-4xl">
      <Info />
      <Alert.Title>Nothing to create a server on yet</Alert.Title>
      <Alert.Description>
        A server needs at least one node and one egg. Add them first.
      </Alert.Description>
    </Alert.Root>
  {/if}
  {#if stages && wizard}
    <!-- -m cancels MultistepForm's p-4; the wrapper gives it the remaining height so only the stage fields scroll. -->
    <div class="-mx-4 -mt-4 min-h-0 grow">
      {#key attempt}
        <MultistepForm
          {stages}
          data={wizard}
          {initStep}
          class="max-h-full min-h-0 w-full max-w-4xl [&>[data-slot=card-content]]:flex [&>[data-slot=card-content]]:min-h-0 [&>[data-slot=card-content]]:flex-col"
          onsubmit={submit}
          cancelHref="/servers"
          submitLabel="Create server"
        />
      {/key}
    </div>
  {:else}
    <div class="mx-auto grid w-full max-w-4xl gap-4" aria-busy="true">
      <Skeleton class="h-8 w-48" />
      <Skeleton class="h-64 w-full" />
    </div>
  {/if}
</div>
