import { z } from 'zod';

export const generalSettings = z.object({
  site_url: z.url('Please enter a valid URL')
});
