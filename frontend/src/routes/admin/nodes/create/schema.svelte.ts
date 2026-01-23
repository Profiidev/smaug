import type { CreateNode } from '$lib/backend/node.svelte';
import type { FormValue } from 'positron-components/components/form/types';
import z from 'zod';

export const units = {
  B: 1,
  KB: 1000,
  KiB: 1024,
  MB: 1000 * 1000,
  MiB: 1024 * 1024,
  GB: 1000 * 1000 * 1000,
  GiB: 1024 * 1024 * 1024,
  TB: 1000 * 1000 * 1000 * 1000,
  TiB: 1024 * 1024 * 1024 * 1024
};

const convertToMB = (amount: number, unitArray: string[]) => {
  return Math.ceil(
    (amount * (units as Record<string, number>)[unitArray[0]]) / (1000 * 1000)
  );
};

export const reformatData = (
  data: FormValue<typeof generalSettings> & FormValue<typeof advancedSettings>
): CreateNode => {
  return {
    name: data.name,
    address: data.address,
    secure: data.secure,
    cpu_limit: data.cpu_unlimit ? undefined : data.cpu_limit,
    memory_limit_mb: data.memory_unlimit
      ? undefined
      : convertToMB(data.memory_limit, data.memory_limit_unit),
    disk_limit_mb: data.storage_unlimit
      ? undefined
      : convertToMB(data.storage_size, data.storage_size_unit)
  };
};

export const generalSettings = z.object({
  name: z.string().min(1, 'Name is required').default(''),
  address: z.string().min(1, 'Address is required').default(''),
  secure: z.boolean().default(true)
});

const amount = z.number().gt(0, 'Amount must be greater than 0');
const unit = z
  .array(z.enum(Object.keys(units)))
  .min(1)
  .max(1)
  .default(['GiB']);

export const advancedSettings = z.object({
  cpu_unlimit: z.boolean().default(true),
  cpu_limit: z.number().gt(1, 'CPU limit must be at least 1').default(1000),
  memory_unlimit: z.boolean().default(true),
  memory_limit: amount.default(2048),
  memory_limit_unit: unit,
  storage_unlimit: z.boolean().default(true),
  storage_size: amount.default(100),
  storage_size_unit: unit
});

export const summary = z.object({
  _phantom_summary: z.string().default('')
});
