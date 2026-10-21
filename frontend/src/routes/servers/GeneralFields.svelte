<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormSelect from '@profidev/pleiades/components/form/form-select.svelte';
  import * as AlertDialog from '@profidev/pleiades/components/ui/alert-dialog';
  import { Badge } from '@profidev/pleiades/components/ui/badge';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import Plus from '@lucide/svelte/icons/plus';
  import Trash from '@lucide/svelte/icons/trash';
  import type {
    FormValue,
    SuperForm
  } from '@profidev/pleiades/components/form/types';
  import type { OptionEgg, ServerOptions } from '$lib/client';
  import {
    DEFAULT_PORT,
    findEgg,
    type FieldsMode,
    type generalStage
  } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof generalStage>>;
    disabled: boolean;
    options: ServerOptions;
    mode: FieldsMode;
    /** Ask before switching eggs, because the switch discards edited variables. */
    variablesEdited?: boolean;
    /** Called after the egg changed (and the user confirmed). */
    oneggchange?: (egg: OptionEgg | undefined) => void;
    /** Disables the admin-only fields (owner, egg, allocations). */
    locked?: boolean;
  }

  let {
    variablesEdited = false,
    oneggchange,
    locked = false,
    options,
    mode,
    ...props
  }: Props = $props();

  let formData = $derived(props.formData.form);
  let adminDisabled = $derived(props.disabled || locked);
  let errors = $derived(props.formData.errors);

  const named = (list: { name: string; uuid: string }[]) =>
    list.map((n) => ({ label: n.name, value: n.uuid }));

  // The egg the other sections were loaded for; a pick that is cancelled reverts to it.
  let currentEgg: string | undefined = $state($formData.egg[0]);
  let pendingEgg: string | undefined = $state();
  let confirmOpen = $state(false);

  const applyEgg = (uuid: string | undefined) => {
    currentEgg = uuid;
    $formData.egg = uuid ? [uuid] : [];
    oneggchange?.(findEgg(options, uuid));
  };

  const onEggSelect = (selected: string[]) => {
    const [uuid] = selected;
    if (uuid === currentEgg) return;
    if (currentEgg && variablesEdited) {
      $formData.egg = [currentEgg];
      pendingEgg = uuid;
      confirmOpen = true;
      return;
    }
    applyEgg(uuid);
  };

  const confirmEgg = () => {
    confirmOpen = false;
    applyEgg(pendingEgg);
  };

  const addAllocation = () => {
    const last = $formData.allocations.at(-1);
    $formData.allocations = [
      ...$formData.allocations,
      { ip: last?.ip ?? '', port: (last?.port ?? DEFAULT_PORT - 1) + 1 }
    ];
  };

  const removeAllocation = (index: number) => {
    $formData.allocations = $formData.allocations.filter((_, i) => i !== index);
  };

  let listError = $derived(
    ($errors.allocations as { _errors?: string[] } | undefined)?._errors?.[0]
  );
</script>

<div class="grid gap-4 md:grid-cols-2">
  <div class="grid gap-1">
    <FormInput {...props} key="name" label="Name" placeholder="survival-smp" />
    <p class="text-muted-foreground text-xs">Up to 191 characters.</p>
  </div>
  <div class="grid gap-1">
    <FormSelect
      {...props}
      key="owner"
      label="Owner"
      single
      disabled={adminDisabled}
      data={named(options.users)}
    />
    <p class="text-muted-foreground text-xs">
      The user who owns the server and can manage it.
    </p>
  </div>
  <div class="grid gap-1">
    <FormSelect
      {...props}
      key="node"
      label="Node"
      single
      disabled={props.disabled || mode === 'edit'}
      data={named(options.nodes)}
    />
    <p class="text-muted-foreground text-xs">
      {mode === 'edit'
        ? 'Use Transfer on the Status tab.'
        : 'Where the server runs. The IPs you type must belong to this node.'}
    </p>
  </div>
  <div class="grid gap-1">
    <FormSelect
      {...props}
      key="egg"
      label="Egg"
      single
      disabled={adminDisabled}
      data={named(options.eggs)}
      onSelectChange={onEggSelect}
    />
    <p class="text-muted-foreground text-xs">
      {mode === 'edit'
        ? 'Changing the egg reloads variables, command and image. A reinstall is recommended.'
        : 'Variables, startup command and image are loaded from the egg.'}
    </p>
  </div>
</div>

<Separator />

<div class="grid gap-2">
  <div class="flex flex-wrap items-center justify-between gap-2">
    <div class="grid gap-0.5">
      <span class="text-sm font-medium">Allocations</span>
      <span class="text-muted-foreground text-xs">
        Type an IP address and a port. The first row is the primary allocation;
        add rows for extra ports. The node checks that each one is free when you
        save.
      </span>
    </div>
    <Button
      class="cursor-pointer"
      size="sm"
      variant="outline"
      type="button"
      disabled={adminDisabled}
      onclick={addAllocation}
    >
      <Plus />
      Add allocation
    </Button>
  </div>
  {#each $formData.allocations as _, i (i)}
    <div class="flex flex-wrap items-start gap-2 sm:flex-nowrap">
      <Badge
        variant={i === 0 ? 'default' : 'outline'}
        class="mt-1.5 w-16 flex-none"
      >
        {i === 0 ? 'Primary' : 'Extra'}
      </Badge>
      <div class="min-w-0 flex-[3_1_200px]">
        <FormInput
          {...props}
          key={`allocations[${i}].ip`}
          aria-label="IP address"
          disabled={adminDisabled}
          class="font-mono"
          placeholder="0.0.0.0"
        />
      </div>
      <span class="text-muted-foreground mt-1.5 hidden sm:block">:</span>
      <div class="min-w-0 flex-[1_1_100px]">
        <FormInput
          {...props}
          key={`allocations[${i}].port`}
          aria-label="Port"
          disabled={adminDisabled}
          type="number"
          class="font-mono"
          placeholder="25565"
        />
      </div>
      <Button
        size="icon-sm"
        variant="destructive"
        type="button"
        class="mt-0.5 cursor-pointer"
        aria-label="Remove allocation"
        disabled={adminDisabled || $formData.allocations.length <= 1}
        onclick={() => removeAllocation(i)}
      >
        <Trash />
      </Button>
    </div>
  {/each}
  {#if listError}
    <p class="text-destructive text-sm">{listError}</p>
  {/if}
</div>

<AlertDialog.Root bind:open={confirmOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Change egg?</AlertDialog.Title>
      <AlertDialog.Description>
        The variables you edited are replaced with the new egg's defaults. The
        startup command and image reload too.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Keep egg</AlertDialog.Cancel>
      <AlertDialog.Action variant="destructive" onclick={confirmEgg}>
        Change egg
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
