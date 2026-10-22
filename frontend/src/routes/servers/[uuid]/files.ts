import { sha256 } from '@noble/hashes/sha2.js';
import { bytesToHex } from '@noble/hashes/utils.js';
import type { FileEntry } from '$lib/client';

/** The editor refuses bigger files (the backend answers `413`). */
export const EDIT_LIMIT = 1024 * 1024;
/** Container path the server root is shown as. */
export const ROOT_LABEL = '/home/container';
/** Upload chunks in flight per file. */
export const PARALLEL_CHUNKS = 4;

const TEXT_EXTENSIONS = new Set([
  'yml',
  'yaml',
  'json',
  'properties',
  'txt',
  'cfg',
  'conf',
  'ini',
  'toml',
  'xml',
  'log',
  'sh',
  'md',
  'env',
  'js',
  'ts',
  'lua',
  'csv'
]);
const TEXT_MIMES = new Set([
  'application/json',
  'application/xml',
  'application/toml',
  'application/yaml',
  'application/x-yaml',
  'application/javascript',
  'application/x-sh'
]);

export const extension = (name: string) => {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : '';
};

/** Text-like regular files up to 1 MiB open in the editor; the rest is download only. */
export const isEditable = (
  entry: Pick<FileEntry, 'kind' | 'mime' | 'name' | 'size'>
) =>
  entry.kind === 'file' &&
  entry.size <= EDIT_LIMIT &&
  // Dotfiles like `.env` have no extension, the name is the type.
  (TEXT_EXTENSIONS.has(
    extension(entry.name) || entry.name.slice(1).toLowerCase()
  ) ||
    entry.mime?.startsWith('text/') === true ||
    TEXT_MIMES.has(entry.mime ?? ''));

/** Folders first, then by name. */
export const sortEntries = (entries: FileEntry[]) =>
  entries.toSorted(
    (a, b) =>
      Number(b.kind === 'dir') - Number(a.kind === 'dir') ||
      a.name.localeCompare(b.name, undefined, { numeric: true })
  );

/** Normalizes `.`, `..`, duplicate slashes; never climbs above `/`. */
export const normalize = (path: string) => {
  const out: string[] = [];
  for (const part of path.split('/')) {
    if (part === '..') {
      out.pop();
    } else if (part && part !== '.') {
      out.push(part);
    }
  }
  return `/${out.join('/')}`;
};

export const joinPath = (dir: string, name: string) =>
  normalize(`${dir}/${name}`);

/** User input relative to `dir` (`../x`, `sub/x`), or absolute (`/x`). */
export const resolvePath = (dir: string, input: string) =>
  normalize(input.startsWith('/') ? input : `${dir}/${input}`);

/** Breadcrumb segments with the path each one opens. */
export const segments = (path: string) => {
  const parts = normalize(path).split('/').filter(Boolean);
  return parts.map((name, i) => ({
    name,
    path: `/${parts.slice(0, i + 1).join('/')}`
  }));
};

/** Readable query value: keep slashes unescaped. */
export const encodePath = (path: string) =>
  encodeURIComponent(path).replaceAll('%2F', '/');

export const formatBytes = (bytes: number) => {
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return unit === 0 ? `${value} B` : `${value.toFixed(1)} ${units[unit]}`;
};

const rtf = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });
const STEPS: [Intl.RelativeTimeFormatUnit, number][] = [
  ['year', 365 * 24 * 3600],
  ['month', 30 * 24 * 3600],
  ['day', 24 * 3600],
  ['hour', 3600],
  ['minute', 60]
];

export const relativeTime = (date: Date, now = Date.now()) => {
  const seconds = (date.getTime() - now) / 1000;
  for (const [unit, size] of STEPS) {
    if (Math.abs(seconds) >= size) {
      return rtf.format(Math.round(seconds / size), unit);
    }
  }
  return 'just now';
};

export type EditorLanguage = 'yaml' | 'json' | 'properties' | 'text';

export const LANGUAGES: Record<EditorLanguage, string> = {
  json: 'JSON',
  properties: 'Properties',
  text: 'Plain text',
  yaml: 'YAML'
};

export const languageFor = (name: string): EditorLanguage => {
  const ext = extension(name);
  if (ext === 'yml' || ext === 'yaml') {
    return 'yaml';
  }
  if (ext === 'json') {
    return 'json';
  }
  if (
    ext === 'properties' ||
    ext === 'cfg' ||
    ext === 'ini' ||
    ext === 'conf'
  ) {
    return 'properties';
  }
  return 'text';
};

/** Line numbers (1-based) whose text differs from the original at the same line. */
export const changedLines = (original: string, current: string) => {
  const a = original.split('\n');
  const b = current.split('\n');
  const out: number[] = [];
  for (let i = 0; i < b.length; i += 1) {
    if (a[i] !== b[i]) {
      out.push(i + 1);
    }
  }
  return out;
};

/** `[start, end)` byte ranges; every chunk but the last is `chunkSize`. */
export const chunkRanges = (size: number, chunkSize: number) => {
  const out: [number, number][] = [];
  for (let start = 0; start < size; start += chunkSize) {
    out.push([start, Math.min(start + chunkSize, size)]);
  }
  return out;
};

/**
 * Sends `file` in chunks, at most `parallel` in flight, and returns the hex
 * sha256 of the whole file. Chunks are read in order so the hash stays
 * incremental; sends may finish in any order.
 */
export const sendChunks = async (
  file: Blob,
  chunkSize: number,
  send: (index: number, chunk: Uint8Array) => Promise<void>,
  parallel = PARALLEL_CHUNKS
) => {
  const hash = sha256.create();
  const inFlight = new Set<Promise<void>>();
  let failed = false;
  let firstError: unknown = undefined;
  try {
    for (const [index, [start, end]] of chunkRanges(
      file.size,
      chunkSize
    ).entries()) {
      if (failed) {
        break;
      }
      // Reads stay sequential so the hash is incremental; `inFlight` bounds the sends.
      // oxlint-disable-next-line no-await-in-loop
      const chunk = new Uint8Array(await file.slice(start, end).arrayBuffer());
      hash.update(chunk);
      if (inFlight.size >= parallel) {
        // oxlint-disable-next-line no-await-in-loop -- backpressure: wait for a free send slot
        await Promise.race(inFlight);
      }
      if (failed) {
        break;
      }
      // Sends never reject: the first error is kept and rethrown below.
      const p: Promise<void> = send(index, chunk)
        .catch((error: unknown) => {
          if (!failed) {
            failed = true;
            firstError = error;
          }
        })
        .finally(() => {
          inFlight.delete(p);
        });
      inFlight.add(p);
    }
  } finally {
    await Promise.all(inFlight);
  }
  if (failed) {
    throw firstError;
  }
  return bytesToHex(hash.digest());
};
