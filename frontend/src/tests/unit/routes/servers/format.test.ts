import { describe, expect, it } from 'vitest';
import {
  formatAllocation,
  formatBytes,
  formatClock,
  formatRate,
  formatUptime,
  percent
} from '$routes/servers/format';

describe('formatAllocation', () => {
  it('joins IPv4 and brackets IPv6', () => {
    expect(formatAllocation({ ip: '10.0.0.5', port: 25_565 })).toBe(
      '10.0.0.5:25565'
    );
    expect(formatAllocation({ ip: '::1', port: 1 })).toBe('[::1]:1');
  });
});

describe('formatBytes', () => {
  it.each([
    [0, '0 B'],
    [1023, '1023 B'],
    [1024, '1 KiB'],
    [1536, '1.5 KiB'],
    [2.1 * 1024 ** 3, '2.1 GiB'],
    [10 * 1024 ** 3, '10 GiB'],
    [12.6 * 1024 ** 2, '13 MiB'],
    [3 * 1024 ** 5, '3072 TiB']
  ])('%d -> %s', (n, text) => {
    expect(formatBytes(n)).toBe(text);
  });
});

describe('formatRate', () => {
  it.each([
    [0, '0 B/s'],
    [999, '999 B/s'],
    [3072, '3.1 KB/s'],
    [14_336, '14 KB/s'],
    [40_000, '40 KB/s'],
    [2_500_000, '2.5 MB/s']
  ])('%d -> %s', (n, text) => {
    expect(formatRate(n)).toBe(text);
  });
});

describe('formatUptime', () => {
  const m = 60_000;
  const h = 60 * m;
  const d = 24 * h;
  it.each([
    [undefined, '—'],
    [0, '—'],
    [-5, '—'],
    [45_000, '45s'],
    [12 * m, '12m'],
    [h + 5 * m, '1h 5m'],
    [3 * d + 4 * h + 12 * m, '3d 4h 12m'],
    [2 * d, '2d 0h 0m']
  ])('%s -> %s', (ms, text) => {
    expect(formatUptime(ms)).toBe(text);
  });
});

describe('formatClock', () => {
  it('prints local HH:MM:SS', () => {
    expect(formatClock(new Date(2026, 0, 1, 9, 4, 1))).toBe('09:04:01');
  });
});

describe('percent', () => {
  it('rounds and treats 0 as unlimited', () => {
    expect(percent(760, 2000)).toBe(38);
    expect(percent(5, 0)).toBe(0);
  });
});
