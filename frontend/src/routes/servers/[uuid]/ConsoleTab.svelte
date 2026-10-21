<script lang="ts">
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import { Badge } from '@profidev/pleiades/components/ui/badge';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import * as Card from '@profidev/pleiades/components/ui/card';
  import * as InputGroup from '@profidev/pleiades/components/ui/input-group';
  import { Progress } from '@profidev/pleiades/components/ui/progress';
  import { Separator } from '@profidev/pleiades/components/ui/separator';
  import { CopyButton } from '@profidev/pleiades/components/ui-extra/copy-button';
  import { toast } from '@profidev/pleiades/components/util/general';
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import CircleX from '@lucide/svelte/icons/circle-x';
  import Info from '@lucide/svelte/icons/info';
  import Play from '@lucide/svelte/icons/play';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import Square from '@lucide/svelte/icons/square';
  import {
    serverCommand,
    serverPower,
    type PowerAction,
    type ServerInfo
  } from '$lib/client';
  import { powerActions } from '../status';
  import {
    formatAllocation,
    formatBytes,
    formatClock,
    formatRate,
    formatUptime,
    percent
  } from '../format';
  import MetricChart from './MetricChart.svelte';
  import type { ConsoleLine, ServerLive } from './live.svelte';
  import { parseAnsi, stripAnsi, type AnsiSegment } from './ansi';

  interface Props {
    server: ServerInfo;
    live: ServerLive;
  }

  let { server, live }: Props = $props();

  const POWER: {
    action: PowerAction;
    label: string;
    icon: typeof Play;
    variant: 'default' | 'outline' | 'destructive';
  }[] = [
    { action: 'start', icon: Play, label: 'Start', variant: 'default' },
    {
      action: 'restart',
      icon: RefreshCw,
      label: 'Restart',
      variant: 'outline'
    },
    { action: 'stop', icon: Square, label: 'Stop', variant: 'outline' },
    { action: 'kill', icon: CircleX, label: 'Kill', variant: 'destructive' }
  ];

  let status = $derived(server.status);
  let canWrite = $derived(
    server.access === 'admin' || server.access === 'write'
  );
  let allowed = $derived(powerActions(status));
  let running = $derived(status === 'running');
  let offline = $derived(status === 'offline' || status === 'crashed');
  let installing = $derived(status === 'installing');
  let address = $derived(
    server.allocations[0] ? formatAllocation(server.allocations[0]) : '—'
  );
  let stats = $derived(live.stats);

  let powerBusy: PowerAction | undefined = $state();
  let command = $state('');
  let log: HTMLDivElement | undefined = $state();
  /** Follow new lines while scrolled to the bottom. */
  let stick = $state(true);

  let powerHint = $derived(
    allowed.length === 0
      ? 'Power actions are locked.'
      : offline
        ? 'Start the server to use the console.'
        : 'Stop sends a graceful stop. Kill ends the process at once and can lose data.'
  );
  let placeholder = $derived(
    installing
      ? 'Commands are unavailable during install'
      : !canWrite
        ? 'Commands need write access'
        : running
          ? 'Type a command'
          : status === 'starting'
            ? 'Waiting for the server to start'
            : 'Server must be running'
  );
  let commandOff = $derived(!running || !canWrite);

  let disk = $derived({
    limit: stats?.disk_limit_bytes ?? 0,
    used: stats?.disk_bytes ?? 0
  });

  let cpu = $derived.by(() => {
    const now = stats?.cpu_millicores ?? 0;
    const limit = stats?.cpu_limit_millicores ?? 0;
    return {
      description: offline
        ? 'Server is offline'
        : limit
          ? `${percent(now, limit)}% of ${limit} millicores`
          : 'No CPU limit',
      // Chart in percent of one core (1000m), Docker-style; exceeds 100 on multi-core.
      series: live.cpu.map((v) => v / 10),
      value: `${now}m`
    };
  });

  let memory = $derived.by(() => {
    const now = stats?.memory_bytes ?? 0;
    const limit = stats?.memory_limit_bytes ?? 0;
    return {
      description: offline
        ? 'Server is offline'
        : limit
          ? `${percent(now, limit)}% of ${formatBytes(limit)} limit`
          : 'No memory limit',
      value: formatBytes(now)
    };
  });

  let network = $derived({
    description: offline
      ? 'Server is offline'
      : `Outbound ↑ ${formatRate(stats?.net_tx_bps ?? 0)}`,
    value: `↓ ${formatRate(stats?.net_rx_bps ?? 0)}`
  });

  /** Base colour; explicit ANSI colours on segments override it. */
  const lineClass = ({ stream, line: raw }: ConsoleLine) => {
    const line = stripAnsi(raw);
    if (stream === 'command') return 'text-muted-foreground';
    if (stream === 'daemon') return 'text-[oklch(0.7_0.14_250)]';
    if (/\b(ERROR|SEVERE|FATAL)\b|Exception/.test(line))
      return 'text-[oklch(0.65_0.22_25)]';
    if (/\bWARN(ING)?\b/.test(line)) return 'text-[oklch(0.8_0.15_85)]';
    return '';
  };

  const prefix = { command: '> ', daemon: '[Daemon] ' } as const;
  const lineSegments = ({ stream, line }: ConsoleLine): AnsiSegment[] =>
    stream in prefix
      ? [{ text: prefix[stream as keyof typeof prefix] }, ...parseAnsi(line)]
      : parseAnsi(line);

  // Keep the newest line in view unless the user scrolled up.
  $effect(() => {
    void live.lines;
    if (log && stick) log.scrollTop = log.scrollHeight;
  });

  const onscroll = () => {
    if (!log) return;
    stick = log.scrollHeight - log.scrollTop - log.clientHeight < 8;
  };

  const jumpToLatest = () => {
    stick = true;
    if (log) log.scrollTop = log.scrollHeight;
  };

  const power = async (action: PowerAction) => {
    powerBusy = action;
    const res = await serverPower({
      body: { action },
      path: { uuid: server.uuid }
    });
    powerBusy = undefined;
    if (!res.response?.ok) {
      toast.error(
        res.response?.status === 409
          ? "The server can't do that right now."
          : "Couldn't send the power action."
      );
    }
  };

  const send = async (e: SubmitEvent) => {
    e.preventDefault();
    const line = command.trim();
    if (!line || commandOff) return;
    command = '';
    live.echo(line);
    const res = await serverCommand({
      body: { line },
      path: { uuid: server.uuid }
    });
    if (!res.response?.ok) {
      toast.error(
        res.response?.status === 409
          ? 'The server is not running.'
          : "Couldn't send the command."
      );
    }
  };
</script>

<div class="grid gap-4">
  {#if status === 'install_failed'}
    <Alert.Root variant="destructive">
      <CircleAlert />
      <Alert.Title>Install failed</Alert.Title>
      <Alert.Description>
        The install script exited with an error. Check the egg's install script,
        then reinstall.
      </Alert.Description>
    </Alert.Root>
  {:else if status === 'suspended'}
    <Alert.Root>
      <Info />
      <Alert.Title>This server is suspended</Alert.Title>
      <Alert.Description>
        It was stopped by an administrator and cannot be started. Files and
        backups stay readable.
      </Alert.Description>
    </Alert.Root>
  {:else if installing}
    <Alert.Root>
      <Info />
      <Alert.Title>The server is installing</Alert.Title>
      <Alert.Description>
        The install script is running and its output is shown below. Commands
        and power actions unlock when it finishes.
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

  <div class="grid gap-4 lg:grid-cols-[320px_minmax(0,1fr)]">
    <div class="grid min-w-0 content-start gap-4">
      <Card.Root>
        <Card.Header>
          <Card.Title>Server</Card.Title>
          <Card.Description>Live details from the node.</Card.Description>
        </Card.Header>
        <Card.Content class="grid gap-3">
          <div class="grid gap-1">
            <span class="text-muted-foreground text-xs">Address</span>
            <div class="flex items-center gap-2">
              <span class="min-w-0 grow font-mono text-sm break-all">
                {address}
              </span>
              <CopyButton
                text={address}
                variant="outline"
                size="icon-sm"
                class="cursor-pointer"
                aria-label="Copy address"
              />
            </div>
            <span class="text-muted-foreground text-xs">
              Primary allocation. Only an admin can change it.
            </span>
          </div>
          <Separator />
          <dl class="grid grid-cols-2 gap-3">
            <div class="grid gap-0.5">
              <dt class="text-muted-foreground text-xs">Uptime</dt>
              <dd class="font-mono text-sm">
                {running ? formatUptime(stats?.uptime_ms) : '—'}
              </dd>
            </div>
            <div class="grid gap-0.5">
              <dt class="text-muted-foreground text-xs">Node</dt>
              <dd class="font-mono text-sm break-all">{server.node.name}</dd>
            </div>
          </dl>
        </Card.Content>
      </Card.Root>

      {#if canWrite}
        <Card.Root>
          <Card.Header>
            <Card.Title>Power</Card.Title>
            <Card.Description>Control the server process.</Card.Description>
          </Card.Header>
          <Card.Content class="grid gap-2.5">
            <div class="grid grid-cols-2 gap-2">
              {#each POWER as { action, icon: Icon, label, variant } (action)}
                <Button
                  {variant}
                  class="cursor-pointer"
                  disabled={!allowed.includes(action) || !!powerBusy}
                  onclick={() => power(action)}
                >
                  <Icon />
                  {label}
                </Button>
              {/each}
            </div>
            <p class="text-muted-foreground text-xs">{powerHint}</p>
          </Card.Content>
        </Card.Root>
      {/if}

      <Card.Root>
        <Card.Header>
          <Card.Title>Disk</Card.Title>
          <Card.Description>Files and logs on the node.</Card.Description>
          <Card.Action>
            <span class="font-mono text-sm font-semibold">
              {formatBytes(disk.used)}
            </span>
          </Card.Action>
        </Card.Header>
        <Card.Content class="grid gap-1.5">
          <Progress
            value={percent(disk.used, disk.limit)}
            aria-label="Disk usage"
          />
          <div class="text-muted-foreground flex justify-between text-xs">
            <span>
              {disk.limit
                ? `${percent(disk.used, disk.limit)}% used`
                : 'Unlimited'}
            </span>
            {#if disk.limit}
              <span class="font-mono">of {formatBytes(disk.limit)}</span>
            {/if}
          </div>
        </Card.Content>
      </Card.Root>
    </div>

    <Card.Root class="min-w-0">
      <Card.Header>
        <Card.Title>Console</Card.Title>
        <Card.Description>
          {installing
            ? 'Live install output. Commands are unavailable until the install finishes.'
            : status === 'suspended'
              ? 'Unavailable while suspended.'
              : 'Live output. Commands go to the server process.'}
        </Card.Description>
        <Card.Action>
          <Badge variant="outline" class="gap-1.5">
            <span
              class={[
                'size-2 shrink-0 rounded-full',
                live.connected
                  ? 'bg-[oklch(0.72_0.17_150)]'
                  : 'bg-muted-foreground'
              ]}
              aria-hidden="true"
            ></span>
            {live.connected
              ? installing
                ? 'Install log'
                : 'Connected'
              : 'Disconnected'}
          </Badge>
        </Card.Action>
      </Card.Header>
      <Card.Content class="flex grow flex-col gap-3">
        <div class="relative flex grow flex-col">
          <div
            bind:this={log}
            {onscroll}
            role="log"
            aria-live="off"
            aria-label="Console output"
            class="ansi bg-background min-h-75 grow basis-0 overflow-y-auto rounded-lg border px-3 py-2.5 font-mono text-xs leading-4.5 lg:min-h-105"
          >
            {#each live.lines as l (l.id)}
              <div class="flex gap-2.5">
                <span class="text-muted-foreground shrink-0">
                  {formatClock(l.at)}
                </span>
                <span
                  class={[
                    'min-w-0 break-all whitespace-pre-wrap',
                    lineClass(l)
                  ]}
                >
                  {#each lineSegments(l) as seg, i (i)}
                    <span
                      style:color={seg.fg}
                      style:background-color={seg.bg}
                      style:font-weight={seg.bold ? 'bold' : undefined}
                      style:opacity={seg.dim ? 0.6 : undefined}
                      style:font-style={seg.italic ? 'italic' : undefined}
                      style:text-decoration={seg.underline
                        ? 'underline'
                        : undefined}>{seg.text}</span
                    >
                  {/each}
                </span>
              </div>
            {/each}
          </div>
          {#if !stick}
            <Button
              size="sm"
              variant="secondary"
              class="absolute right-3 bottom-3 cursor-pointer"
              onclick={jumpToLatest}
            >
              <ArrowDown />
              Jump to latest
            </Button>
          {/if}
        </div>
        <form class="flex gap-2" onsubmit={send}>
          <InputGroup.Root class="min-w-0 grow" data-disabled={commandOff}>
            <InputGroup.Addon>
              <span class="font-mono">&gt;</span>
            </InputGroup.Addon>
            <InputGroup.Input
              class="font-mono"
              bind:value={command}
              {placeholder}
              disabled={commandOff}
              aria-label="Console command"
              autocomplete="off"
            />
          </InputGroup.Root>
          <Button
            type="submit"
            class="cursor-pointer"
            disabled={commandOff}
            aria-label="Send command"
          >
            <ArrowUp />
            Send
          </Button>
        </form>
      </Card.Content>
    </Card.Root>
  </div>

  <div class="grid gap-4 lg:grid-cols-3">
    <MetricChart title="CPU" {...cpu} format={(v) => `${Math.round(v)}%`} />
    <MetricChart
      title="Memory"
      {...memory}
      series={live.memory}
      color="var(--chart-1)"
      format={formatBytes}
    />
    <MetricChart
      title="Network"
      {...network}
      series={live.rx}
      series2={live.tx}
      legend={['Inbound', 'Outbound']}
      format={formatRate}
    />
  </div>
</div>

<style>
  /* ANSI 0-15, readable on the console background in each theme. */
  .ansi {
    --ansi-0: #27272a;
    --ansi-1: #dc2626;
    --ansi-2: #16a34a;
    --ansi-3: #a16207;
    --ansi-4: #2563eb;
    --ansi-5: #c026d3;
    --ansi-6: #0891b2;
    --ansi-7: #71717a;
    --ansi-8: #52525b;
    --ansi-9: #ef4444;
    --ansi-10: #22c55e;
    --ansi-11: #ca8a04;
    --ansi-12: #3b82f6;
    --ansi-13: #d946ef;
    --ansi-14: #06b6d4;
    --ansi-15: #a1a1aa;
  }

  :global(.dark) .ansi {
    --ansi-0: #71717a;
    --ansi-1: #f87171;
    --ansi-2: #4ade80;
    --ansi-3: #facc15;
    --ansi-4: #60a5fa;
    --ansi-5: #e879f9;
    --ansi-6: #22d3ee;
    --ansi-7: #d4d4d8;
    --ansi-8: #a1a1aa;
    --ansi-9: #fca5a5;
    --ansi-10: #86efac;
    --ansi-11: #fde047;
    --ansi-12: #93c5fd;
    --ansi-13: #f0abfc;
    --ansi-14: #67e8f9;
    --ansi-15: #fafafa;
  }
</style>
