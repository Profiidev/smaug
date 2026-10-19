import { describe, expect, it } from 'vitest';
import {
  advancedSettings,
  generalSettings,
  reformatData,
  summary,
  undoReformatData,
  units
} from '$routes/nodes/create/schema.svelte';
import type { NodeInfo } from '$lib/client';

type Form = Parameters<typeof reformatData>[0];

const form = (over: Partial<Form> = {}): Form => ({
  address: 'wings.example.com',
  cpu_limit: 200,
  cpu_unlimit: false,
  memory_limit: 2,
  memory_limit_unit: ['GiB'],
  memory_unlimit: false,
  name: 'Alpha',
  secure: true,
  storage_size: 100,
  storage_size_unit: ['GB'],
  storage_unlimit: false,
  ...over
});

const node: NodeInfo = {
  address: 'wings.example.com',
  connected: false,
  id: 'n1',
  name: 'Alpha',
  port: 443,
  secure: true,
  token: 'tok'
};

describe('units', () => {
  it('maps SI and binary units to bytes', () => {
    expect(units.KB).toBe(1000);
    expect(units.KiB).toBe(1024);
    expect(units.GB).toBe(1e9);
    expect(units.GiB).toBe(1024 ** 3);
    expect(units.TiB).toBe(1024 ** 4);
  });
});

describe('generalSettings', () => {
  it('accepts a name and address and defaults secure to true', () => {
    const r = generalSettings.safeParse({ address: 'a', name: 'n' });
    expect(r.success).toBe(true);
    expect(r.data?.secure).toBe(true);
  });

  it.each([
    ['name', { address: 'a', name: '' }],
    ['address', { address: '', name: 'n' }]
  ])('requires a %s', (field, input) => {
    const r = generalSettings.safeParse(input);
    expect(r.success).toBe(false);
    expect(r.error?.issues[0].path).toEqual([field]);
  });
});

describe('advancedSettings', () => {
  it('defaults to unlimited resources', () => {
    const r = advancedSettings.parse({});
    expect(r).toEqual({
      cpu_limit: 1000,
      cpu_unlimit: true,
      memory_limit: 2048,
      memory_limit_unit: ['MiB'],
      memory_unlimit: true,
      storage_size: 100,
      storage_size_unit: ['GiB'],
      storage_unlimit: true
    });
  });

  it.each([
    ['cpu_limit at 1', { cpu_limit: 1 }],
    ['zero memory', { memory_limit: 0 }],
    ['negative storage', { storage_size: -1 }],
    ['an unknown unit', { memory_limit_unit: ['PB'] }],
    ['no unit', { storage_size_unit: [] }],
    ['two units', { storage_size_unit: ['GB', 'MB'] }]
  ])('rejects %s', (_name, input) => {
    expect(advancedSettings.safeParse(input).success).toBe(false);
  });

  it('accepts the smallest valid limits', () => {
    const r = advancedSettings.safeParse({
      cpu_limit: 1.01,
      memory_limit: 0.5,
      storage_size: 1
    });
    expect(r.success).toBe(true);
  });
});

describe('summary', () => {
  it('only carries a phantom field', () => {
    expect(summary.parse({})).toEqual({ _phantom_summary: '' });
  });
});

describe('reformatData', () => {
  it('converts the limits to MB', () => {
    expect(reformatData(form())).toEqual({
      address: 'wings.example.com',
      cpu_limit: 200,
      disk_limit_mb: 100_000,
      memory_limit_mb: (2 * 1024 ** 3) / 1e6,
      name: 'Alpha',
      secure: true
    });
  });

  it('drops every limit that is unlimited', () => {
    const r = reformatData(
      form({ cpu_unlimit: true, memory_unlimit: true, storage_unlimit: true })
    );
    expect(r.cpu_limit).toBeUndefined();
    expect(r.memory_limit_mb).toBeUndefined();
    expect(r.disk_limit_mb).toBeUndefined();
  });
});

describe('undoReformatData', () => {
  it('fills unlimited nodes with the default limits', () => {
    expect(undoReformatData(node)).toEqual({
      address: 'wings.example.com:443',
      cpu_limit: 1000,
      cpu_unlimit: true,
      // `selectUnit` keeps the last divisible unit in `units` key order, so
      // 2 GiB / 100 GiB come back as MiB.
      memory_limit: 2048,
      memory_limit_unit: ['MiB'],
      memory_unlimit: true,
      name: 'Alpha',
      secure: true,
      storage_size: 102_400,
      storage_size_unit: ['MiB'],
      storage_unlimit: true
    });
  });

  it('picks the last unit in key order that divides the limit evenly', () => {
    const r = undoReformatData({
      ...node,
      cpu_limit: 300,
      disk_limit_mb: 5_000_000,
      memory_limit_mb: 1
    });
    expect(r).toMatchObject({
      cpu_limit: 300,
      cpu_unlimit: false,
      memory_limit: 1,
      memory_limit_unit: ['MB'],
      memory_unlimit: false,
      storage_size: 5,
      storage_size_unit: ['TB'],
      storage_unlimit: false
    });
  });

  it('round-trips through reformatData', () => {
    const limited = {
      ...node,
      cpu_limit: 4,
      disk_limit_mb: 2048,
      memory_limit_mb: 512
    };
    const r = reformatData(undoReformatData(limited));
    expect(r).toMatchObject({
      cpu_limit: 4,
      disk_limit_mb: 2048,
      memory_limit_mb: 512
    });
  });
});
