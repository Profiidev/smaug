<script lang="ts" generics="T">
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import Ban from '@lucide/svelte/icons/ban';
  import CheckIcon from '@lucide/svelte/icons/check';
  import Plus from '@lucide/svelte/icons/plus';
  import { Badge } from 'positron-components/components/ui/badge';
  import { Button } from 'positron-components/components/ui/button';
  import * as Card from 'positron-components/components/ui/card';
  import { Spinner } from 'positron-components/components/ui/spinner';
  import {
    type Error,
    type FormRecord
  } from 'positron-components/components/form/types';
  import type { Component, SvelteComponent } from 'svelte';
  import type { Stage } from './types.svelte';

  interface Props<T> {
    stages: Stage<T>[];
    onsubmit: (
      data: object
    ) => Error | undefined | void | Promise<Error | undefined | void>;
    data?: T;
    submitLabel?: string;
    submitIcon?: Component;
  }

  let {
    stages,
    onsubmit,
    data = undefined as T,
    submitLabel = 'Create',
    submitIcon: SubmitIcon = Plus
  }: Props<T> = $props();

  let stage = $state(0);
  let form: undefined | SvelteComponent = $state();
  let isLoading = $state(false);

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

      onsubmit(rawData);
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
        {data}
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
              href="/nodes"
            >
              <Ban />
              Cancel
            </Button>
            <Button class="cursor-pointer" type="submit" disabled={isLoading}>
              {#if stage === stages.length - 1}
                {submitLabel}
                {#if isLoading}
                  <Spinner />
                {:else}
                  <SubmitIcon />
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
