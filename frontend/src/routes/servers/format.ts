import type { Allocation } from '$lib/client';

/** `ip:port`, IPv6 in brackets. */
export const formatAllocation = ({ ip, port }: Allocation) =>
  ip.includes(':') ? `[${ip}]:${port}` : `${ip}:${port}`;

/** One decimal below 10, none above; trailing `.0` dropped. */
const short = (n: number) => String(Number(n.toFixed(n < 10 ? 1 : 0)));

const scaled = (value: number, base: number, units: string[]) => {
  let n = value;
  let i = 0;
  while (n >= base && i < units.length - 1) {
    n /= base;
    i += 1;
  }
  return `${short(n)} ${units[i]}`;
};

/** Binary sizes: `2.1 GiB`, `512 MiB`. */
export const formatBytes = (bytes: number) =>
  scaled(bytes, 1024, ['B', 'KiB', 'MiB', 'GiB', 'TiB']);

/** Decimal transfer rates: `14 KB/s`. */
export const formatRate = (bps: number) =>
  scaled(bps, 1000, ['B/s', 'KB/s', 'MB/s', 'GB/s']);

/** `3d 4h 12m`, `12m`, `45s`; `—` when not up. */
export const formatUptime = (ms: number | undefined) => {
  if (!ms || ms < 0) {
    return '—';
  }
  const s = Math.floor(ms / 1000);
  if (s < 60) {
    return `${s}s`;
  }
  const parts = [
    [Math.floor(s / 86_400), 'd'],
    [Math.floor(s / 3600) % 24, 'h'],
    [Math.floor(s / 60) % 60, 'm']
  ] as const;
  const first = parts.findIndex(([n]) => n > 0);
  return parts
    .slice(first)
    .map(([n, u]) => `${n}${u}`)
    .join(' ');
};

/** Local `HH:MM:SS`. */
export const formatClock = (at: Date) => at.toTimeString().slice(0, 8);

/** Percent of `limit`, rounded; 0 when unlimited. */
export const percent = (value: number, limit: number) =>
  limit > 0 ? Math.round((value / limit) * 100) : 0;
