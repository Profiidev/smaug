<script lang="ts">
  import FormSelect from '@profidev/pleiades/components/form/form-select.svelte';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Empty from '@profidev/pleiades/components/ui/empty';
  import Plus from '@lucide/svelte/icons/plus';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import X from '@lucide/svelte/icons/x';
  import type {
    FormValue,
    SuperForm
  } from '@profidev/pleiades/components/form/types';
  import type { ServerOptions } from '$lib/client';
  import type { permissionsStage } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof permissionsStage>>;
    disabled: boolean;
    /** Users and groups that can be granted access. */
    options: Pick<ServerOptions, 'users' | 'groups'>;
  }

  let { options, ...props }: Props = $props();

  let formData = $derived(props.formData.form);

  const ACCESS = [
    { label: 'Read', value: 'read' as const },
    { label: 'Write', value: 'write' as const }
  ];

  const SECTIONS = [
    { key: 'userGrants', kind: 'user', title: 'User', plural: 'users' },
    { key: 'groupGrants', kind: 'group', title: 'Group', plural: 'groups' }
  ] as const;

  type Key = (typeof SECTIONS)[number]['key'];

  /** Entries not picked in another row of the same list. */
  const choices = (key: Key, row: number) => {
    const taken = new Set(
      $formData[key].filter((_, i) => i !== row).map((g) => g.uuid[0])
    );
    return options[key === 'userGrants' ? 'users' : 'groups']
      .filter((e) => !taken.has(e.uuid))
      .map((e) => ({ label: e.name, value: e.uuid }));
  };

  const add = (key: Key) => {
    $formData[key] = [...$formData[key], { access: ['read'], uuid: [] }];
  };

  const remove = (key: Key, index: number) => {
    $formData[key] = $formData[key].filter((_, i) => i !== index);
  };
</script>

<div class="grid gap-0.5">
  <span class="text-sm font-medium">Access grants</span>
  <span class="text-muted-foreground text-xs">
    Read can view the server; write can also change it. The owner and admins
    always have full access.
  </span>
</div>

{#each SECTIONS as { key, kind, title, plural } (key)}
  <section class="grid gap-2" aria-label={`${title} grants`}>
    <div class="flex flex-wrap items-center justify-between gap-2">
      <h3 class="text-sm font-medium">{title}s</h3>
      <Button
        class="cursor-pointer"
        size="sm"
        variant="outline"
        type="button"
        disabled={props.disabled}
        onclick={() => add(key)}
      >
        <Plus />
        Add {kind}
      </Button>
    </div>
    {#if $formData[key].length}
      <div
        class="text-muted-foreground flex gap-2 text-xs font-medium"
        aria-hidden="true"
      >
        <span class="min-w-0 flex-[3_1_200px]">{title}</span>
        <span class="min-w-0 flex-[1_1_120px]">Access</span>
        <span class="w-8 flex-none"></span>
      </div>
    {/if}
    {#each $formData[key] as _, i (i)}
      <!-- FormSelect needs a label for its placeholder; the column header shows it instead. -->
      <div class="flex items-start gap-2 [&_label]:hidden">
        <div
          class="min-w-0 flex-[3_1_200px]"
          role="group"
          aria-label={`${title} ${i + 1}`}
        >
          <FormSelect
            {...props}
            key={`${key}[${i}].uuid`}
            label={kind}
            single
            data={choices(key, i)}
          />
        </div>
        <div
          class="min-w-0 flex-[1_1_120px]"
          role="group"
          aria-label={`Access for ${kind} ${i + 1}`}
        >
          <FormSelect
            {...props}
            key={`${key}[${i}].access`}
            label="access"
            single
            data={ACCESS}
          />
        </div>
        <Button
          size="icon-sm"
          variant="destructive"
          type="button"
          class="mt-0.5 flex-none cursor-pointer"
          aria-label={`Remove ${kind} ${i + 1}`}
          disabled={props.disabled}
          onclick={() => remove(key, i)}
        >
          <X />
        </Button>
      </div>
    {:else}
      <Empty.Root class="border border-dashed">
        <Empty.Header>
          <Empty.Media variant="icon"><ShieldCheck /></Empty.Media>
          <Empty.Title>No {plural}</Empty.Title>
          <Empty.Description>No {kind} has access yet.</Empty.Description>
        </Empty.Header>
      </Empty.Root>
    {/each}
  </section>
{/each}
