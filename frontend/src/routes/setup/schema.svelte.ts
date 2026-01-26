import { z } from 'zod';

export const databaseSetupSchema = z.object({
  disclaimerAccepted: z.boolean().refine((val) => val === true, {
    message: 'You must accept the disclaimer to proceed.'
  })
});
