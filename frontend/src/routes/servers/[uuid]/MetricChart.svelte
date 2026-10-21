<script lang="ts">
  import * as Card from '@profidev/pleiades/components/ui/card';
  import * as Chart from '@profidev/pleiades/components/ui/chart';
  import { AreaChart } from 'layerchart';
  import { curveMonotoneX } from 'd3-shape';

  interface Props {
    title: string;
    /** Header number, e.g. `760m`. */
    value: string;
    description: string;
    series: number[];
    /** Second line (network outbound), with a legend. */
    series2?: number[];
    legend?: [string, string];
    /** Main series colour. */
    color?: string;
    /** Axis and tooltip value format. */
    format: (v: number) => string;
  }

  let {
    title,
    value,
    description,
    series,
    series2,
    legend,
    color = 'var(--chart-2)',
    format
  }: Props = $props();

  let config = $derived({
    a: { color, label: legend?.[0] ?? title },
    b: { color: 'var(--chart-1)', label: legend?.[1] ?? '' }
  } satisfies Chart.ChartConfig);

  let data = $derived(series.map((a, t) => ({ a, b: series2?.[t] ?? 0, t })));

  let chartSeries = $derived([
    { key: 'a', label: config.a.label, color: config.a.color },
    ...(series2
      ? [{ key: 'b', label: config.b.label, color: config.b.color }]
      : [])
  ]);

  // One sample every 2 s; the last one is "now".
  const ago = (t: number) => {
    const s = (series.length - 1 - t) * 2;
    return s ? `${s} s ago` : 'now';
  };
</script>

<Card.Root>
  <Card.Header>
    <Card.Title>{title}</Card.Title>
    <Card.Description>{description}</Card.Description>
    <Card.Action>
      <span class="font-mono text-sm font-semibold">{value}</span>
    </Card.Action>
  </Card.Header>
  <Card.Content class="grid gap-1.5">
    <Chart.Container
      {config}
      class="aspect-auto h-30 w-full"
      role="img"
      aria-label={`${title} over the last 60 seconds`}
    >
      <AreaChart
        {data}
        x="t"
        xDomain={[0, Math.max(series.length - 1, 1)]}
        series={chartSeries}
        seriesLayout="overlap"
        axis="y"
        grid={{ yTicks: 3 }}
        rule={false}
        clip
        padding={{ bottom: 4, left: 56, right: 0, top: 8 }}
        props={{
          yAxis: { format, ticks: 3 },
          area: {
            curve: curveMonotoneX,
            fillOpacity: 0.16,
            line: { strokeWidth: 1.5 }
          }
        }}
      >
        {#snippet tooltip()}
          <Chart.Tooltip labelFormatter={ago}>
            {#snippet formatter({ value: v, name, item })}
              <div
                class="size-2.5 shrink-0 rounded-[2px] bg-(--color-bg)"
                style:--color-bg={item.color}
              ></div>
              <div class="flex flex-1 items-center justify-between gap-2">
                <span class="text-muted-foreground">{name}</span>
                <span class="text-foreground font-mono font-medium">
                  {format(Number(v))}
                </span>
              </div>
            {/snippet}
          </Chart.Tooltip>
        {/snippet}
      </AreaChart>
    </Chart.Container>
    <div class="text-muted-foreground flex justify-between gap-2 text-xs">
      <span>60 s ago</span>
      {#if legend}
        <span class="flex gap-3">
          <span class="flex items-center gap-1">
            <span class="size-2 rounded-full bg-(--chart-2)"></span>
            {legend[0]}
          </span>
          <span class="flex items-center gap-1">
            <span class="size-2 rounded-full bg-(--chart-1)"></span>
            {legend[1]}
          </span>
        </span>
      {/if}
      <span>now</span>
    </div>
  </Card.Content>
</Card.Root>
