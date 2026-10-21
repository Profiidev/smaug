<script lang="ts">
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import FormInput from '@profidev/pleiades/components/form/form-input.svelte';
  import FormSelect from '@profidev/pleiades/components/form/form-select.svelte';
  import type { FormValue } from '@profidev/pleiades/components/form/types';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { invalidate } from '$app/navigation';
  import { z } from 'zod';
  import ApplyErrors from '$lib/components/form/ApplyErrors.svelte';
  import { transferServer, type Named } from '$lib/client';
  import { allocationRow, isFieldErrors } from '../schema.svelte';

  interface Props {
    open: boolean;
    server: { uuid: string; name: string; node: Named };
    /** Every node; the server's current one is left out. */
    nodes: Named[];
  }

  let { open = $bindable(), server, nodes }: Props = $props();

  const schema = allocationRow.extend({
    node: z.array(z.string()).min(1, 'Select a target node.').max(1).default([])
  });

  const FIELDS = new Set(['node', 'ip', 'port']);

  let errors: Record<string, string> | undefined = $state();
  let targets = $derived(
    nodes
      .filter((n) => n.uuid !== server.node.uuid)
      .map((n) => ({ label: n.name, value: n.uuid }))
  );

  const onsubmit = async (form: FormValue<typeof schema>) => {
    const node = nodes.find((n) => n.uuid === form.node[0]);
    const res = await transferServer({
      body: { ip: form.ip.trim(), node: form.node[0], port: form.port },
      path: { uuid: server.uuid }
    });

    if (!res.response?.ok) {
      const status = res.response?.status;
      if (!isFieldErrors(res.error)) {
        return { error: "Couldn't transfer server." };
      }
      const shown: Record<string, string> = {};
      const other: string[] = [];
      for (const { field, message } of res.error.errors) {
        if (FIELDS.has(field)) shown[field] ??= message;
        else other.push(message);
      }
      errors = shown;
      if (other.length) {
        return { error: other.join(' ') };
      }
      return {
        error:
          status === 409
            ? "Couldn't transfer server: the allocation is already in use."
            : "Couldn't transfer server. Fix the highlighted fields."
      };
    }

    toast.success(`Server ${server.name} transferred to ${node?.name}`);
    invalidate(`/api/servers/${server.uuid}`);
  };
</script>

<FormDialog
  title={`Transfer ${server.name}`}
  description="Choose where to move this server."
  confirm="Start transfer"
  {schema}
  {onsubmit}
  bind:open
>
  {#snippet children({ props })}
    <ApplyErrors formData={props.formData} {errors} />
    <FormSelect
      {...props}
      key="node"
      label="Target node"
      single
      data={targets}
    />
    <div class="grid gap-2">
      <span class="text-sm font-medium">Primary allocation on target</span>
      <div class="flex items-start gap-2">
        <div class="min-w-0 flex-[3_1_0]">
          <FormInput
            {...props}
            key="ip"
            aria-label="IP address"
            class="font-mono"
            placeholder="0.0.0.0"
          />
        </div>
        <span class="text-muted-foreground mt-1.5">:</span>
        <div class="min-w-0 flex-[1_1_0]">
          <FormInput
            {...props}
            key="port"
            aria-label="Port"
            type="number"
            class="font-mono"
            placeholder="25565"
          />
        </div>
      </div>
      <p class="text-muted-foreground text-xs">
        Type an IP and port that belong to the target node. Extra allocations
        are removed and can be added again afterwards.
      </p>
    </div>
  {/snippet}
</FormDialog>
