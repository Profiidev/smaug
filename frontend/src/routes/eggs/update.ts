import { editEgg, eggInfo, eggSource } from '$lib/client';
import {
  type EggForm,
  STAGE_KEYS,
  apiErrorMessage,
  formatData,
  toEggBody
} from './schema.svelte';

export const UPDATE_SECTIONS = [
  { key: 'general', keys: STAGE_KEYS[0], title: 'General' },
  { key: 'variables', keys: STAGE_KEYS[1], title: 'Variables' },
  { key: 'install', keys: STAGE_KEYS[2], title: 'Install script' },
  { key: 'runtime', keys: STAGE_KEYS[3], title: 'Runtime' },
  { key: 'config', keys: STAGE_KEYS[4], title: 'Config' }
] as const;

export type SectionKey = (typeof UPDATE_SECTIONS)[number]['key'];

export interface EggUpdate {
  uuid: string;
  /** Installed egg (last saved state). */
  current: EggForm;
  /** Egg served at the update URL. */
  source: EggForm;
  changed: SectionKey[];
}

// The update URL is local config, never replaced by the source.
const sectionKeys = (keys: readonly string[]) =>
  keys.filter((k) => k !== 'updateUrl');

const pick = (value: Record<string, unknown>, keys: readonly string[]) =>
  JSON.stringify(sectionKeys(keys).map((k) => value[k]));

export const changedSections = (
  current: EggForm,
  source: EggForm
): SectionKey[] =>
  UPDATE_SECTIONS.filter(
    (s) => pick(current, s.keys) !== pick(source, s.keys)
  ).map((s) => s.key);

/** `current` with the given sections replaced by the source version. */
export const mergeSections = (
  current: EggForm,
  source: EggForm,
  sections: SectionKey[]
): EggForm => {
  const src: Record<string, unknown> = source;
  const replaced = UPDATE_SECTIONS.filter((s) => sections.includes(s.key))
    .flatMap((s) => sectionKeys(s.keys))
    .map((k) => [k, src[k]]);
  return { ...current, ...Object.fromEntries(replaced) };
};

const parseSource = (raw: unknown) => {
  try {
    return formatData(raw);
  } catch {
    throw new Error('The update URL does not serve a valid egg');
  }
};

/**
 * Fetches the egg's source through the backend and compares it per section.
 * `current` defaults to the saved egg. Throws with a user-facing message.
 */
export const checkEggUpdate = async (
  uuid: string,
  current?: EggForm
): Promise<EggUpdate> => {
  const [info, src] = await Promise.all([
    current ? undefined : eggInfo({ path: { uuid } }),
    eggSource({ path: { uuid } })
  ]);
  if (src.error || !src.data) {
    throw new Error(apiErrorMessage(src.error, 'Could not check for updates'));
  }
  const installed = current ?? (info?.data && formatData(info.data));
  if (!installed) {
    throw new Error('Failed to load egg');
  }
  const source = parseSource(src.data.source);
  return {
    changed: changedSections(installed, source),
    current: installed,
    source,
    uuid
  };
};

/** Saves the egg with the chosen sections taken from the source. Throws with a user-facing message. */
export const applyEggUpdate = async (
  update: EggUpdate,
  sections: SectionKey[]
): Promise<EggForm> => {
  const merged = mergeSections(update.current, update.source, sections);
  const res = await editEgg({
    body: { ...toEggBody(merged), uuid: update.uuid }
  });
  if (res.error) {
    throw new Error(apiErrorMessage(res.error, 'Failed to update egg'));
  }
  return merged;
};

export interface DiffLine {
  kind: 'ctx' | 'add' | 'del';
  text: string;
}

/** Line diff of `a` -> `b`. */
export const lineDiff = (a: string, b: string): DiffLine[] => {
  const x = a.split('\n');
  const y = b.split('\n');
  // Ponytail: O(n·m) LCS table, fine for install scripts; swap for Myers if eggs grow huge
  const lcs = Array.from({ length: x.length + 1 }, () =>
    Array.from({ length: y.length + 1 }, () => 0)
  );
  for (let i = x.length - 1; i >= 0; i -= 1) {
    for (let j = y.length - 1; j >= 0; j -= 1) {
      lcs[i][j] =
        x[i] === y[j]
          ? lcs[i + 1][j + 1] + 1
          : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < x.length || j < y.length) {
    if (i < x.length && j < y.length && x[i] === y[j]) {
      out.push({ kind: 'ctx', text: x[i] });
      i += 1;
      j += 1;
    } else if (
      j >= y.length ||
      (i < x.length && lcs[i + 1][j] >= lcs[i][j + 1])
    ) {
      out.push({ kind: 'del', text: x[i] });
      i += 1;
    } else {
      out.push({ kind: 'add', text: y[j] });
      j += 1;
    }
  }
  return out;
};
