<script lang="ts">
  import FormDialog from '@profidev/pleiades/components/form/form-dialog.svelte';
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Card from '@profidev/pleiades/components/ui/card';
  import * as Item from '@profidev/pleiades/components/ui/item';
  import { Spinner } from '@profidev/pleiades/components/ui/spinner';
  import { toast } from '@profidev/pleiades/components/util/general';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import Ban from '@lucide/svelte/icons/ban';
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import Info from '@lucide/svelte/icons/info';
  import RotateCw from '@lucide/svelte/icons/rotate-cw';
  import Trash from '@lucide/svelte/icons/trash';
  import { invalidate } from '$app/navigation';
  import { z } from 'zod';
  import {
    reinstallServer,
    setServerInstalled,
    setServerSuspended,
    type Allocation,
    type Named,
    type ServerInfo
  } from '$lib/client';
  import StatusBadge from '../StatusBadge.svelte';
  import { STATUS_DESCRIPTIONS, availableActions } from '../status';
  import TransferDialog from './TransferDialog.svelte';

  interface Props {
    server: ServerInfo;
    /** Transfer targets (admins only). */
    nodes: Named[];
    /** Opens the page's delete dialog. */
    ondelete: () => void;
  }

  let { server, nodes, ondelete }: Props = $props();

  let reinstallOpen = $state(false);
  let suspendOpen = $state(false);
  let transferOpen = $state(false);
  let installBusy = $state(false);
  let unsuspendBusy = $state(false);

  let admin = $derived(server.access === 'admin');
  let canWrite = $derived(admin || server.access === 'write');
  let installed = $derived(server.install_state === 'installed');
  let allowed = $derived(availableActions(server.status));
  let can = $derived({
    delete: admin && allowed.delete,
    install: admin && allowed.install,
    reinstall: canWrite && allowed.reinstall,
    suspend: admin && allowed.suspend,
    transfer: admin && allowed.transfer && nodes.length > 1
  });

  const refresh = () => invalidate(`/api/servers/${server.uuid}`);

  const formatAllocation = ({ ip, port }: Allocation) =>
    ip.includes(':') ? `[${ip}]:${port}` : `${ip}:${port}`;

  let details = $derived([
    { label: 'Name', value: server.name },
    { label: 'Node', value: server.node.name },
    {
      label: 'Primary allocation',
      mono: true,
      value: server.allocations[0]
        ? formatAllocation(server.allocations[0])
        : '—'
    },
    {
      label: 'Extra allocations',
      mono: true,
      value: server.allocations.slice(1).map(formatAllocation).join(', ') || '—'
    },
    { label: 'Egg', value: server.egg.name },
    { label: 'Image', mono: true, value: server.image },
    { label: 'UUID', mono: true, value: server.uuid }
  ]);

  const reinstall = async () => {
    const res = await reinstallServer({ path: { uuid: server.uuid } });
    if (!res.response?.ok) {
      return { error: "Couldn't reinstall server." };
    }
    toast.success(`Reinstalling ${server.name}`);
    refresh();
  };

  const toggleInstalled = async () => {
    installBusy = true;
    const res = await setServerInstalled({
      body: { installed: !installed },
      path: { uuid: server.uuid }
    });
    installBusy = false;
    if (!res.response?.ok) {
      toast.error("Couldn't change the install status.");
      return;
    }
    toast.success(
      installed
        ? `${server.name} marked as not installed`
        : `${server.name} marked as installed`
    );
    refresh();
  };

  const setSuspended = async (suspended: boolean) => {
    const res = await setServerSuspended({
      body: { suspended },
      path: { uuid: server.uuid }
    });
    if (!res.response?.ok) {
      return {
        error: suspended
          ? "Couldn't suspend server."
          : "Couldn't unsuspend server."
      };
    }
    toast.success(
      suspended ? `${server.name} suspended` : `${server.name} unsuspended`
    );
    refresh();
  };

  const unsuspend = async () => {
    unsuspendBusy = true;
    const ret = await setSuspended(false);
    unsuspendBusy = false;
    if (ret) toast.error(ret.error);
  };
</script>

<div class="grid gap-4">
  <Card.Root>
    <Card.Header>
      <Card.Title>Status</Card.Title>
      <Card.Description>{STATUS_DESCRIPTIONS[server.status]}</Card.Description>
    </Card.Header>
    <Card.Content class="grid gap-3">
      <div class="flex flex-wrap items-center gap-2">
        <StatusBadge status={server.status} />
        <span class="text-muted-foreground text-xs">
          Created {server.created_at.toLocaleDateString()}
        </span>
      </div>
      {#if server.status === 'installing'}
        <p class="text-muted-foreground text-sm">
          Install progress appears here once the node reports it.
        </p>
      {:else if server.status === 'install_failed'}
        <Alert.Root variant="destructive">
          <CircleAlert />
          <Alert.Title>Install failed</Alert.Title>
          <Alert.Description>
            The install script exited with an error. Check the egg's install
            script, then reinstall.
          </Alert.Description>
        </Alert.Root>
      {:else if server.status === 'suspended'}
        <Alert.Root>
          <Info />
          <Alert.Title>Suspended</Alert.Title>
          <Alert.Description>
            Users cannot open or start this server. Files and allocations are
            kept.
          </Alert.Description>
        </Alert.Root>
      {/if}
      {#if server.error}
        <Alert.Root variant="destructive">
          <CircleAlert />
          <Alert.Title>Node reported an error</Alert.Title>
          <Alert.Description>
            <p class="font-mono text-xs break-all">{server.error}</p>
          </Alert.Description>
        </Alert.Root>
      {/if}
    </Card.Content>
  </Card.Root>

  <div class="grid gap-4 md:grid-cols-2">
    <Card.Root>
      <Card.Header>
        <Card.Title>Usage</Card.Title>
      </Card.Header>
      <Card.Content>
        <!-- TODO(wings): CPU, memory and disk usage once the node reports them. -->
        <p class="text-muted-foreground text-sm">
          Usage is shown while the server is running.
        </p>
      </Card.Content>
    </Card.Root>
    <Card.Root>
      <Card.Header>
        <Card.Title>Details</Card.Title>
      </Card.Header>
      <Card.Content>
        <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2 text-sm">
          {#each details as row (row.label)}
            <dt class="text-muted-foreground">{row.label}</dt>
            <dd class={['break-all', row.mono && 'font-mono']}>{row.value}</dd>
          {/each}
        </dl>
      </Card.Content>
    </Card.Root>
  </div>

  <Card.Root>
    <Card.Header>
      <Card.Title>Actions</Card.Title>
      <Card.Description>
        {#if canWrite}
          These change the server's state. Most of them stop the server first.
        {:else}
          You have view-only access. Actions need write access to this server.
        {/if}
      </Card.Description>
    </Card.Header>
    <Card.Content class="grid gap-2">
      <Item.Root variant="outline">
        <Item.Content>
          <Item.Title>Reinstall server</Item.Title>
          <Item.Description>
            Stops the server and runs the egg's install script again. Existing
            files can be overwritten.
          </Item.Description>
        </Item.Content>
        <Item.Actions>
          <Button
            variant="outline"
            size="sm"
            class="cursor-pointer"
            disabled={!can.reinstall}
            onclick={() => (reinstallOpen = true)}
          >
            <RotateCw />
            Reinstall
          </Button>
        </Item.Actions>
      </Item.Root>
      <Item.Root variant="outline">
        <Item.Content>
          <Item.Title>Install status</Item.Title>
          <Item.Description>
            {installed
              ? 'Marked as installed. Mark it not installed to block starts until you reinstall.'
              : 'Not marked as installed. Mark it installed if the files are already in place.'}
          </Item.Description>
        </Item.Content>
        <Item.Actions>
          <Button
            variant="outline"
            size="sm"
            class="cursor-pointer"
            disabled={!can.install || installBusy}
            onclick={toggleInstalled}
          >
            {#if installBusy}<Spinner />{/if}
            {installed ? 'Mark as not installed' : 'Mark as installed'}
          </Button>
        </Item.Actions>
      </Item.Root>
      <Item.Root variant="outline">
        <Item.Content>
          <Item.Title>{server.suspended ? 'Unsuspend' : 'Suspend'}</Item.Title>
          <Item.Description>
            {server.suspended
              ? 'Lets users open and start the server again.'
              : 'Stops the server and blocks users from opening it. Files are kept.'}
          </Item.Description>
        </Item.Content>
        <Item.Actions>
          {#if server.suspended}
            <Button
              variant="outline"
              size="sm"
              class="cursor-pointer"
              disabled={!can.suspend || unsuspendBusy}
              onclick={unsuspend}
            >
              {#if unsuspendBusy}<Spinner />{/if}
              Unsuspend
            </Button>
          {:else}
            <Button
              variant="outline"
              size="sm"
              class="cursor-pointer"
              disabled={!can.suspend}
              onclick={() => (suspendOpen = true)}
            >
              <Ban />
              Suspend
            </Button>
          {/if}
        </Item.Actions>
      </Item.Root>
      <Item.Root variant="outline">
        <Item.Content>
          <Item.Title>Transfer</Item.Title>
          <Item.Description>
            Moves the server to another node. You type a new primary allocation.
          </Item.Description>
        </Item.Content>
        <Item.Actions>
          <Button
            variant="outline"
            size="sm"
            class="cursor-pointer"
            disabled={!can.transfer}
            onclick={() => (transferOpen = true)}
          >
            <ArrowRight />
            Transfer…
          </Button>
        </Item.Actions>
      </Item.Root>
    </Card.Content>
  </Card.Root>

  <Card.Root>
    <Card.Header>
      <Card.Title>Danger zone</Card.Title>
    </Card.Header>
    <Card.Content>
      <Item.Root variant="outline">
        <Item.Content>
          <Item.Title>Delete server</Item.Title>
          <Item.Description>
            Permanently removes the server and its files from the node. This
            cannot be undone.
          </Item.Description>
        </Item.Content>
        <Item.Actions>
          <Button
            variant="destructive"
            size="sm"
            class="cursor-pointer"
            disabled={!can.delete}
            onclick={ondelete}
          >
            <Trash />
            Delete server
          </Button>
        </Item.Actions>
      </Item.Root>
    </Card.Content>
  </Card.Root>
</div>

<FormDialog
  title={`Reinstall ${server.name}?`}
  description="The server stops and the egg's install script runs again. Existing files can be overwritten."
  confirm="Reinstall"
  confirmVariant="destructive"
  onsubmit={reinstall}
  bind:open={reinstallOpen}
  schema={z.object({})}
/>

<FormDialog
  title={`Suspend ${server.name}?`}
  description="The server stops and users cannot open it until it is unsuspended. Files and allocations are kept."
  confirm="Suspend"
  confirmVariant="destructive"
  onsubmit={() => setSuspended(true)}
  bind:open={suspendOpen}
  schema={z.object({})}
/>

{#if admin}
  <TransferDialog bind:open={transferOpen} {server} {nodes} />
{/if}
