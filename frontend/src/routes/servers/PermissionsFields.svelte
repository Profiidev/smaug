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

  let targets = $derived([
    {
      items: options.users.map((u) => ({
        label: u.name,
        value: `user:${u.uuid}`
      })),
      label: 'Users'
    },
    {
      items: options.groups.map((g) => ({
        label: g.name,
        value: `group:${g.uuid}`
      })),
      label: 'Groups'
    }
  ]);

  const ACCESS = [
    { label: 'Read', value: 'read' as const },
    { label: 'Write', value: 'write' as const }
  ];

  const addGrant = () => {
    $formData.grants = [...$formData.grants, { access: ['read'], target: [] }];
  };

  const removeGrant = (index: number) => {
    $formData.grants = $formData.grants.filter((_, i) => i !== index);
  };
</script>

<div class="flex flex-wrap items-center justify-between gap-2">
  <div class="grid gap-0.5">
    <span class="text-sm font-medium">Access grants</span>
    <span class="text-muted-foreground text-xs">
      Read can view the server; write can also change it. The owner and admins
      always have full access.
    </span>
  </div>
  <Button
    size="sm"
    variant="outline"
    type="button"
    disabled={props.disabled}
    onclick={addGrant}
  >
    <Plus />
    Add grant
  </Button>
</div>

{#each $formData.grants as _, i (i)}
  <div class="flex flex-wrap items-start gap-2 sm:flex-nowrap">
    <div class="min-w-0 flex-[3_1_200px]">
      <FormSelect
        {...props}
        key={`grants[${i}].target`}
        label="User or group"
        single
        data={targets}
      />
    </div>
    <div class="min-w-0 flex-[1_1_120px]">
      <FormSelect
        {...props}
        key={`grants[${i}].access`}
        label="Access"
        single
        data={ACCESS}
      />
    </div>
    <Button
      size="icon-sm"
      variant="destructive"
      type="button"
      class="mt-6"
      aria-label="Remove grant"
      disabled={props.disabled}
      onclick={() => removeGrant(i)}
    >
      <X />
    </Button>
  </div>
{:else}
  <Empty.Root class="border border-dashed">
    <Empty.Header>
      <Empty.Media variant="icon"><ShieldCheck /></Empty.Media>
      <Empty.Title>No grants</Empty.Title>
      <Empty.Description>
        Only the owner and admins can access this server.
      </Empty.Description>
    </Empty.Header>
  </Empty.Root>
{/each}
