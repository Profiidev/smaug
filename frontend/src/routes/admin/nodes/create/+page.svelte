<script lang="ts">
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import Ban from '@lucide/svelte/icons/ban';
  import CheckIcon from '@lucide/svelte/icons/check';
  import Plus from '@lucide/svelte/icons/plus';
  import { Badge } from 'positron-components/components/ui/badge';
  import { Button } from 'positron-components/components/ui/button';
  import * as Card from 'positron-components/components/ui/card';
  import { Spinner } from 'positron-components/components/ui/spinner';
  import BaseForm from 'positron-components/components/form/base-form.svelte';
  import { type FormRecord } from 'positron-components/components/form/types';
  import { reformatData } from './schema.svelte';
  import { goto } from '$app/navigation';
  import type {
    Component,
    ComponentProps,
    Snippet,
    SvelteComponent
  } from 'svelte';
  import GeneralSettings from './GeneralSettings.svelte';
  import AdvancedSettings from './AdvancedSettings.svelte';
  import { createNode } from '$lib/backend/node.svelte';
  import { RequestError } from 'positron-components/backend';
  import { toast } from 'positron-components/components/util/general';

  interface StageProps {
    initialValue?: any;
    onsubmit: ComponentProps<typeof BaseForm>['onsubmit'];
    footer: Snippet<[{ isLoading: boolean }]>;
    isLoading: boolean;
  }

  type StageComponent = Component<
    StageProps,
    { getValue: () => object | undefined }
  >;

  interface Stage {
    title: string;
    content: StageComponent;
    data: object;
  }

  let stage = $state(0);
  let form: undefined | SvelteComponent = $state();
  let isLoading = $state(false);

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

  const gotoStep = (step: number) => {
    stages[stage].data = form?.getValue() || {};
    stage = step;
  };

  const submit = async (form: FormRecord) => {
    stages[stage].data = form;
    if (stage < stages.length - 1) {
      stage += 1;
    } else {
      let rawData = stages
        // last element is summary
        .filter((_, i) => i < stages.length)
        .reduce((acc, s) => ({ ...acc, ...s.data }), {});

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
    }
    return undefined;
  };
</script>

<div class="flex h-full items-center justify-center p-4">
  <Card.Root class="w-120">
    <Card.Header class="flex flex-col gap-4">
      <div class="flex gap-2">
        {#each stages as _, index}
          <Badge
            class={'flex size-6 rounded-full' +
              (stage > index ? ' cursor-pointer p-0' : '')}
            variant={stage === index ? 'default' : 'outline'}
            onclick={() => {
              if (stage > index) {
                gotoStep(index);
              }
            }}
          >
            {#if stage > index}
              <CheckIcon />
            {:else}
              {index + 1}
            {/if}
          </Badge>
        {/each}
      </div>
      <Card.Title>{stages[stage].title}</Card.Title>
    </Card.Header>
    <Card.Content>
      {@const current = stages[stage]}
      <current.content
        bind:this={form}
        initialValue={current.data}
        onsubmit={submit}
        bind:isLoading
      >
        {#snippet footer({ isLoading })}
          <Card.Footer class="w-full gap-2 px-0">
            <Button
              class="cursor-pointer"
              variant="outline"
              disabled={stage === 0 || isLoading}
              onclick={() => {
                if (stage > 0) {
                  gotoStep(stage - 1);
                }
              }}
            >
              <ArrowLeft />
              Previous
            </Button>
            <Button
              class="ml-auto cursor-pointer"
              variant="outline"
              disabled={isLoading}
              href="/admin/nodes"
            >
              <Ban />
              Cancel
            </Button>
            <Button class="cursor-pointer" type="submit" disabled={isLoading}>
              {#if stage === stages.length - 1}
                Create
                {#if isLoading}
                  <Spinner />
                {:else}
                  <Plus />
                {/if}
              {:else}
                Next
                <ArrowRight />
              {/if}
            </Button>
          </Card.Footer>
        {/snippet}
      </current.content>
    </Card.Content>
  </Card.Root>
</div>
