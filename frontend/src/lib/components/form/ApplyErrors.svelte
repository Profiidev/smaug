<script lang="ts" generics="T extends FormRecord">
  // Shows server-side field errors on a superform. Render it inside a
  // BaseForm's children snippet; it applies `errors` whenever the object changes.
  import type {
    FormRecord,
    SuperForm
  } from '@profidev/pleiades/components/form/types';

  interface Props {
    formData: SuperForm<T>;
    /** Dotted path (`allocations.0.ip`, `allocations._errors`) -> message. */
    errors?: Record<string, string>;
  }

  let { formData, errors }: Props = $props();

  type Tree = Record<string, unknown>;

  $effect(() => {
    if (!errors) return;
    const entries = Object.entries(errors);
    formData.errors.update((current) => {
      const next: Tree = structuredClone(current as Tree);
      for (const [path, message] of entries) {
        const keys = path.split('.');
        const last = keys.pop()!;
        let node = next;
        for (const key of keys) {
          if (typeof node[key] !== 'object' || node[key] === null) {
            node[key] = {};
          }
          node = node[key] as Tree;
        }
        node[last] = [message];
      }
      return next as typeof current;
    });
  });
</script>
