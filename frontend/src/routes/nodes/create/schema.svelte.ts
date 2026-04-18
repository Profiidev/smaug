import type { CreateNode, NodeInfo } from '$lib/backend/node.svelte';
import type { FormValue } from '@profidev/pleiades/components/form/types';
import z from 'zod';

export const units = {
  B: 1,
  GB: 1000 * 1000 * 1000,
  GiB: 1024 * 1024 * 1024,
  KB: 1000,
  KiB: 1024,
  MB: 1000 * 1000,
  MiB: 1024 * 1024,
  TB: 1000 * 1000 * 1000 * 1000,
  TiB: 1024 * 1024 * 1024 * 1024
};

const convertToMB = (amount: number, unitArray: string[]) => (
    (amount * (units as Record<string, number>)[unitArray[0]]) / (1000 * 1000)
  );

export const reformatData = (
  data: FormValue<typeof generalSettings> & FormValue<typeof advancedSettings>
): CreateNode => ({
    address: data.address,
    cpu_limit: data.cpu_unlimit ? undefined : data.cpu_limit,
    disk_limit_mb: data.storage_unlimit
      ? undefined
      : convertToMB(data.storage_size, data.storage_size_unit),
    memory_limit_mb: data.memory_unlimit
      ? undefined
      : convertToMB(data.memory_limit, data.memory_limit_unit),
    name: data.name,
    secure: data.secure
  });

const selectUnit = (mb: number): [number, string[]] => {
  const bytes = mb * 1000 * 1000;
  let unit = 'B';
  let unit_amount = bytes;

  for (const [key, value] of Object.entries(units)) {
    if (bytes % value === 0) {
      unit = key;
      unit_amount = bytes / value;
    }
  }

  return [unit_amount, [unit]];
};

export const undoReformatData = (
  node: NodeInfo
): FormValue<typeof generalSettings> & FormValue<typeof advancedSettings> => {
  const [storage_size, storage_unit] = selectUnit(
    node.disk_limit_mb ?? (100 * units.GiB) / units.MB
  );
  const [memory_limit, memory_unit] = selectUnit(
    node.memory_limit_mb ?? (2 * units.GiB) / units.MB
  );

  return {
    address: `${node.address}:${node.port}`,
    cpu_limit: node.cpu_limit ?? 1000,
    cpu_unlimit: !node.cpu_limit,
    memory_limit,
    memory_limit_unit: memory_unit,
    memory_unlimit: !node.memory_limit_mb,
    name: node.name,
    secure: node.secure,
    storage_size,
    storage_size_unit: storage_unit,
    storage_unlimit: !node.disk_limit_mb
  };
};

export const generalSettings = z.object({
  address: z.string().min(1, 'Address is required').default(''),
  name: z.string().min(1, 'Name is required').default(''),
  secure: z.boolean().default(true)
});

const amount = z.number().gt(0, 'Amount must be greater than 0');
const unit = z
  .array(z.enum(Object.keys(units)))
  .min(1)
  .max(1);

export const advancedSettings = z.object({
  cpu_limit: z.number().gt(1, 'CPU limit must be at least 1').default(1000),
  cpu_unlimit: z.boolean().default(true),
  memory_limit: amount.default(2048),
  memory_limit_unit: unit.default(['MiB']),
  memory_unlimit: z.boolean().default(true),
  storage_size: amount.default(100),
  storage_size_unit: unit.default(['GiB']),
  storage_unlimit: z.boolean().default(true)
});

export const summary = z.object({
  _phantom_summary: z.string().default('')
});
