<script lang="ts">
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import { Switch } from '@profidev/pleiades/components/ui/switch';
  import * as Item from '@profidev/pleiades/components/ui/item';
  import * as Form from '@profidev/pleiades/components/ui/form';
  import type {
    FormValue,
    SuperForm
  } from '@profidev/pleiades/components/form/types';
  import type { resourcesStage } from './schema.svelte';

  interface Props {
    formData: SuperForm<FormValue<typeof resourcesStage>>;
    disabled: boolean;
  }

  let { ...props }: Props = $props();

  let formData = $derived(props.formData.form);
  let errors = $derived(props.formData.errors);

  const LIMITS = [
    {
      hint: '1000 millicores is one core.',
      key: 'cpu',
      label: 'CPU',
      switchKey: 'cpuUnlimited',
      unit: 'millicores'
    },
    {
      hint: 'Memory the container may use.',
      key: 'memory',
      label: 'Memory',
      switchKey: 'memoryUnlimited',
      unit: 'MiB'
    },
    {
      hint: 'Disk space for the server files.',
      key: 'disk',
      label: 'Storage',
      switchKey: 'diskUnlimited',
      unit: 'MiB'
    }
  ] as const;
</script>

{#each LIMITS as limit (limit.key)}
  <div class="grid gap-2">
    <Item.Root variant="outline" size="sm">
      <Item.Content>
        <Item.Title>Unlimited {limit.label}</Item.Title>
        <Item.Description>
          {$formData[limit.switchKey]
            ? `No ${limit.label.toLowerCase()} limit.`
            : limit.hint}
        </Item.Description>
      </Item.Content>
      <Item.Actions>
        <Switch
          aria-label="Unlimited {limit.label}"
          disabled={props.disabled}
          bind:checked={$formData[limit.switchKey]}
        />
      </Item.Actions>
    </Item.Root>
    {#if !$formData[limit.switchKey]}
      <div class="md:max-w-xs">
        <FormInput
          {...props}
          key={limit.key}
          label="{limit.label} limit ({limit.unit})"
          type="number"
          min={0}
        />
      </div>
    {:else if $errors[limit.key]}
      <!-- A server error on a hidden limit still needs to be seen. -->
      <Form.Field form={props.formData} name={limit.key}>
        <Form.FieldErrors />
      </Form.Field>
    {/if}
  </div>
{/each}

<div class="grid gap-1 md:max-w-xs">
  <FormInput
    {...props}
    key="backupLimit"
    label="Backup limit"
    type="number"
    min={0}
  />
  <p class="text-muted-foreground text-xs">0 turns backups off.</p>
</div>
