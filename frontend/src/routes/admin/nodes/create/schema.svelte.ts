import type { FormRecord } from 'positron-components/components/form/types';
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

export const reformatData = (data: FormRecord): any => {
  let storage_size = data.storage_size as number;
  let storage_size_unit = (data.storage_size_unit as string[])[0];
  let storage_mb = Math.ceil(
    (storage_size * (units as Record<string, number>)[storage_size_unit]) /
      (1000 * 1000)
  );

  return {
    name: data.name as string,
    address: data.address as string,
    storage_mb
  };
};

export const generalInformation = z.object({
  name: z.string().min(1, 'Name is required').default(''),
  address: z.string().min(1, 'Address is required').default(''),
  secure: z.boolean().default(true)
});

export const resources = z.object({
  storage_size: z
    .number()
    .gt(0, 'Storage size must be greater than 0')
    .default(100),
  storage_size_unit: z
    .array(z.enum(Object.keys(units)))
    .min(1)
    .max(1)
    .default(['GiB'])
});

export const summary = z.object({
  _phantom_summary: z.string().default('')
});
