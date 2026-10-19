// See https://svelte.dev/docs/kit/types#app.d.ts
import type { EggImport } from './routes/eggs/schema.svelte';

// For information about these interfaces
declare global {
  namespace App {
    // Interface Error {}
    // Interface Locals {}
    // Interface PageData {}
    interface PageState {
      /** An egg file imported on the list page, handed to /eggs/create. */
      eggImport?: EggImport;
    }
    // Interface Platform {}
  }
}
