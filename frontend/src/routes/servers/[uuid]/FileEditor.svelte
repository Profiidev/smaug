<script lang="ts">
  import { untrack } from 'svelte';
  import { basicSetup } from 'codemirror';
  import {
    Compartment,
    EditorState,
    RangeSetBuilder,
    StateEffect,
    type Extension
  } from '@codemirror/state';
  import {
    Decoration,
    EditorView,
    ViewPlugin,
    type DecorationSet,
    type ViewUpdate
  } from '@codemirror/view';
  import {
    HighlightStyle,
    StreamLanguage,
    syntaxHighlighting
  } from '@codemirror/language';
  import { tags } from '@lezer/highlight';
  import { yaml } from '@codemirror/lang-yaml';
  import { json } from '@codemirror/lang-json';
  import { properties } from '@codemirror/legacy-modes/mode/properties';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import Save from '@lucide/svelte/icons/save';
  import Download from '@lucide/svelte/icons/download';
  import { Button } from '@profidev/pleiades/components/ui/button';
  import { Badge } from '@profidev/pleiades/components/ui/badge';
  import { Skeleton } from '@profidev/pleiades/components/ui/skeleton';
  import { Spinner } from '@profidev/pleiades/components/ui/spinner';
  import * as Alert from '@profidev/pleiades/components/ui/alert';
  import * as Select from '@profidev/pleiades/components/ui/select';
  import { toast } from '@profidev/pleiades/components/util/general';
  import { beforeNavigate } from '$app/navigation';
  import { fileContent, writeFileContent } from '$lib/client';
  import {
    changedLines,
    formatBytes,
    languageFor,
    LANGUAGES,
    ROOT_LABEL,
    type EditorLanguage
  } from './files';

  interface Props {
    uuid: string;
    /** Full path of the file, from the server root. */
    path: string;
    backHref: string;
    downloadHref: string;
    canWrite: boolean;
  }

  let { uuid, path, backHref, downloadHref, canWrite }: Props = $props();

  let name = $derived(path.slice(path.lastIndexOf('/') + 1));
  let dir = $derived(path.slice(0, path.lastIndexOf('/') + 1));

  /** Saved file content; undefined while loading. */
  let original: string | undefined = $state.raw();
  let current = $state.raw('');
  let loadError: 'too_large' | 'other' | undefined = $state();
  let saving = $state(false);
  // Writable: the syntax select overrides the guess until the file changes.
  let language: EditorLanguage = $derived(languageFor(path));
  let line = $state(1);
  let col = $state(1);
  let view: EditorView | undefined;

  let dirty = $derived(original !== undefined && current !== original);
  let eol = $derived(original?.includes('\r\n') ? 'CRLF' : 'LF');
  let bytes = $derived(new TextEncoder().encode(current).length);

  const languageExt = (lang: EditorLanguage): Extension =>
    lang === 'yaml'
      ? yaml()
      : lang === 'json'
        ? json()
        : lang === 'properties'
          ? StreamLanguage.define(properties)
          : [];

  const languageComp = new Compartment();
  /** Recomputes the changed-line tint after save / discard. */
  const refresh = StateEffect.define<null>();
  const changedLine = Decoration.line({ class: 'cm-changed-line' });

  const tintChanged = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(v: EditorView) {
        this.decorations = this.build(v);
      }
      update(u: ViewUpdate) {
        if (
          u.docChanged ||
          u.transactions.some((t) => t.effects.some((e) => e.is(refresh)))
        )
          this.decorations = this.build(u.view);
      }
      build(v: EditorView) {
        const builder = new RangeSetBuilder<Decoration>();
        // ponytail: compares line by line at the same index, an inserted
        // line tints everything below it; a real diff if that bothers users.
        for (const n of changedLines(original ?? '', v.state.doc.toString()))
          builder.add(
            v.state.doc.line(n).from,
            v.state.doc.line(n).from,
            changedLine
          );
        return builder.finish();
      }
    },
    { decorations: (p) => p.decorations }
  );

  const highlight = HighlightStyle.define([
    {
      color: 'var(--syntax-key)',
      tag: [
        tags.propertyName,
        tags.definition(tags.propertyName),
        tags.definition(tags.variableName),
        tags.heading
      ]
    },
    {
      color: 'var(--syntax-string)',
      tag: [tags.string, tags.special(tags.string), tags.quote, tags.content]
    },
    {
      color: 'var(--syntax-number)',
      tag: [tags.number, tags.integer, tags.float]
    },
    { color: 'var(--syntax-bool)', tag: [tags.bool, tags.null, tags.atom] },
    {
      color: 'var(--muted-foreground)',
      tag: [tags.comment, tags.lineComment, tags.blockComment]
    }
  ]);

  const theme = EditorView.theme({
    '&': { backgroundColor: 'transparent', color: 'var(--foreground)' },
    '&.cm-focused': { outline: 'none' },
    '.cm-content': { caretColor: 'var(--foreground)' },
    '.cm-scroller': { fontFamily: 'var(--font-mono)', lineHeight: '1.6' },
    '.cm-gutters': {
      backgroundColor: 'transparent',
      border: 'none',
      color: 'var(--muted-foreground)'
    },
    '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'transparent' },
    '.cm-changed-line': { backgroundColor: 'var(--syntax-changed)' },
    '.cm-cursor': { borderLeftColor: 'var(--foreground)' },
    '&.cm-focused .cm-selectionBackground, .cm-selectionBackground': {
      backgroundColor: 'var(--accent)'
    }
  });

  $effect(() => {
    const target = path;
    original = undefined;
    loadError = undefined;
    fileContent({
      parseAs: 'text',
      path: { uuid },
      query: { path: target }
    }).then((res) => {
      if (target !== path) return;
      if (!res.response?.ok) {
        loadError = res.response?.status === 413 ? 'too_large' : 'other';
        return;
      }
      // `parseAs: 'text'` yields a string, the generated type says Blob.
      original = res.data as unknown as string;
      current = original;
    });
  });

  const mount = (el: HTMLElement) =>
    untrack(() => {
      view = new EditorView({
        parent: el,
        state: EditorState.create({
          doc: original,
          extensions: [
            basicSetup,
            EditorState.lineSeparator.of(eol === 'CRLF' ? '\r\n' : '\n'),
            EditorState.readOnly.of(!canWrite),
            languageComp.of(languageExt(language)),
            syntaxHighlighting(highlight),
            theme,
            tintChanged,
            EditorView.contentAttributes.of({
              'aria-label': `Contents of ${name}`
            }),
            EditorView.updateListener.of((u) => {
              if (u.docChanged) current = u.state.doc.toString();
              const head = u.state.selection.main.head;
              const l = u.state.doc.lineAt(head);
              line = l.number;
              col = head - l.from + 1;
            })
          ]
        })
      });
      return () => {
        view?.destroy();
        view = undefined;
      };
    });

  const setLanguage = (value: string) => {
    language = value as EditorLanguage;
    view?.dispatch({
      effects: languageComp.reconfigure(languageExt(language))
    });
  };

  const discard = () => {
    if (!view || original === undefined) return;
    view.dispatch({
      changes: { from: 0, insert: original, to: view.state.doc.length }
    });
  };

  const save = async () => {
    if (!dirty || saving || !canWrite) return;
    saving = true;
    const text = current;
    const res = await writeFileContent({
      body: text,
      path: { uuid },
      query: { path }
    });
    saving = false;
    if (!res.response?.ok) {
      toast.error(
        res.response?.status === 413
          ? 'File is too large to save from the editor'
          : 'Failed to save file'
      );
      return;
    }
    original = text;
    view?.dispatch({ effects: refresh.of(null) });
    toast.success(`Saved ${name}`);
  };

  const onkeydown = (e: KeyboardEvent) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
      e.preventDefault();
      save();
    }
  };

  beforeNavigate((nav) => {
    if (!dirty) return;
    // `leave` (tab close, reload) shows the browser's own prompt on cancel.
    if (nav.type === 'leave' || !confirm('Discard unsaved changes?'))
      nav.cancel();
  });
</script>

<svelte:window {onkeydown} />

<div class="editor grid gap-3">
  <div class="flex flex-wrap items-center gap-2">
    <Button variant="ghost" size="sm" href={backHref}>
      <ArrowLeft />
      Files
    </Button>
    <span class="text-muted-foreground min-w-0 truncate font-mono text-sm">
      {ROOT_LABEL}{dir}<span class="text-foreground font-medium">{name}</span>
    </span>
    {#if dirty}
      <Badge variant="secondary">Unsaved changes</Badge>
    {/if}
    <div class="ml-auto flex flex-wrap items-center gap-2">
      <Select.Root type="single" value={language} onValueChange={setLanguage}>
        <Select.Trigger aria-label="Syntax" class="cursor-pointer">
          {LANGUAGES[language]}
        </Select.Trigger>
        <Select.Content>
          {#each Object.entries(LANGUAGES) as [value, label] (value)}
            <Select.Item {value}>{label}</Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>
      {#if canWrite}
        <Button
          variant="outline"
          class="cursor-pointer"
          disabled={!dirty || saving}
          onclick={discard}
        >
          Discard
        </Button>
        <Button
          class="cursor-pointer"
          disabled={!dirty || saving}
          onclick={save}
        >
          {#if saving}<Spinner />{:else}<Save />{/if}
          Save content
        </Button>
      {/if}
    </div>
  </div>

  {#if loadError}
    <Alert.Root variant="destructive">
      <Alert.Title>
        {loadError === 'too_large'
          ? 'File is too large to edit'
          : 'Failed to load file'}
      </Alert.Title>
      <Alert.Description>
        {loadError === 'too_large'
          ? 'Files over 1 MiB can only be downloaded.'
          : 'The node may be offline.'}
      </Alert.Description>
      <Button
        variant="outline"
        size="sm"
        class="mt-2 w-fit"
        href={downloadHref}
        download={name}
        data-sveltekit-reload
      >
        <Download />
        Download
      </Button>
    </Alert.Root>
  {:else if original === undefined}
    <div class="grid gap-2 rounded-xl border p-4">
      {#each [1, 2, 3, 4, 5, 6] as i (i)}
        <Skeleton class="h-4" style={`width: ${30 + ((i * 17) % 50)}%`} />
      {/each}
    </div>
  {:else}
    <div class="bg-card overflow-hidden rounded-xl border">
      <div
        class="max-h-[70vh] min-h-64 overflow-auto py-2 text-sm"
        {@attach mount}
      ></div>
      <div
        class="text-muted-foreground flex flex-wrap justify-between gap-2 border-t px-4 py-2 font-mono text-xs"
      >
        <span>{LANGUAGES[language]} · UTF-8 · {eol}</span>
        <span>Ln {line}, Col {col} · {formatBytes(bytes)}</span>
      </div>
    </div>
    <p class="text-muted-foreground text-xs">
      {canWrite
        ? 'Ctrl+S saves. Changes to this file take effect after the server reloads or restarts.'
        : 'View only. You need write access to change this file.'}
    </p>
  {/if}
</div>

<style>
  .editor {
    --syntax-key: oklch(0.55 0.17 255);
    --syntax-string: oklch(0.55 0.15 150);
    --syntax-number: oklch(0.62 0.17 55);
    --syntax-bool: oklch(0.55 0.2 305);
    --syntax-changed: oklch(0.8 0.15 85 / 18%);
  }
  :global(.dark) .editor {
    --syntax-key: oklch(0.72 0.13 250);
    --syntax-string: oklch(0.78 0.15 150);
    --syntax-number: oklch(0.8 0.14 65);
    --syntax-bool: oklch(0.75 0.15 310);
    --syntax-changed: oklch(0.75 0.15 85 / 15%);
  }
</style>
