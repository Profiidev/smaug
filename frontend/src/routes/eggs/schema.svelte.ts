/* oxlint-disable unicorn/no-null -- the egg JSON format uses null for absent description, icon, script, update_url, features and config fields */
import type {
  FormRecord,
  FormValue,
  SuperForm
} from '@profidev/pleiades/components/form/types';
import { z } from 'zod';
import {
  type AnyEgg,
  EGG_VERSIONS,
  type EggConfigFile,
  type EggConfigFindScalar,
  type EggConfigFindValue,
  type EggConfigLogs,
  type EggConfigRaw,
  type EggConfigStartup,
  type EggVersion,
  type LegacyVariable,
  PARSERS,
  PELICAN_COMMENT,
  PTERODACTYL_COMMENT,
  type PelicanEggV3,
  type PelicanVariable,
  type PterodactylEggV2,
  RESERVED_ENV_NAMES
} from '$lib/egg';
import type { EggBody } from '$lib/client';

export const PARSER_OPTIONS = PARSERS.map((v) => ({ label: v, value: v }));

export const FIND_MODES = ['string', 'number', 'boolean', 'map'] as const;
export const FIND_MODE_OPTIONS = FIND_MODES.map((v) => ({
  label: v,
  value: v
}));

export const KNOWN_FEATURES = [
  {
    description:
      "Shows a modal when Mojang's EULA message appears in console (Minecraft Java eggs).",
    formKey: 'featureEula',
    key: 'eula',
    label: 'EULA prompt'
  },
  {
    description:
      'Shows a modal on UnsupportedClassVersionError, pointing at the docker image dropdown.',
    formKey: 'featureJavaVersion',
    key: 'java_version',
    label: 'Java version mismatch'
  },
  {
    description:
      "Shows a modal when the container hits the node's process-count (PID) limit.",
    formKey: 'featurePidLimit',
    key: 'pid_limit',
    label: 'PID limit'
  },
  {
    description:
      'Shows a modal when a Steam Game Server Login Token is invalid or missing.',
    formKey: 'featureGslToken',
    key: 'gsl_token',
    label: 'GSLT token'
  },
  {
    description:
      'Shows a modal when SteamCMD runs out of disk space during install/update.',
    formKey: 'featureSteamDiskSpace',
    key: 'steam_disk_space',
    label: 'Steam disk space'
  }
] as const;

/** Shown under each section title in the create wizard and on the edit page. */
export const SECTION_DESCRIPTIONS = {
  config: 'Files the panel edits before each start.',
  general: 'Basic details, icon and feature flags.',
  install: 'The container and script that install the server files.',
  runtime:
    'Docker images, startup commands and how the panel detects a running server.',
  variables: 'Environment variables users can configure on a server.'
} as const;

export const dockerImageRow = z.object({
  image: z.string().min(1, 'Image reference is required').default(''),
  label: z.string().min(1, 'Label is required').default('')
});

export const startupCommandRow = z.object({
  command: z.string().min(1, 'Command is required').default(''),
  label: z.string().min(1, 'Label is required').default('')
});

export const findMapRow = z.object({
  match: z.string().default(''),
  replace: z.string().default('')
});

export const findEntryRow = z.object({
  key: z.string().min(1, 'Key is required').default(''),
  map: z.array(findMapRow).default([]),
  mode: z.array(z.enum(FIND_MODES)).min(1).max(1).default(['string']),
  value: z.string().default('')
});

export const configFileRow = z.object({
  find: z.array(findEntryRow).default([]),
  parser: z.array(z.enum(PARSERS)).min(1).max(1).default(['file']),
  path: z.string().min(1, 'File path is required').default('')
});

export const variableRow = z.object({
  defaultValue: z.string().default(''),
  description: z.string().default(''),
  envVariable: z
    .string()
    .min(1, 'Env variable is required')
    .max(255, 'Max 255 characters')
    .regex(/^[A-Za-z0-9_-]+$/, 'Letters, numbers, dash and underscore only')
    .default(''),
  name: z
    .string()
    .min(1, 'Name is required')
    .max(255, 'Max 255 characters')
    .default(''),
  rules: z
    .array(z.string())
    .min(1, 'At least one validation rule is required')
    .default([]),
  userEditable: z.boolean().default(false),
  userViewable: z.boolean().default(false)
});

const RESERVED = new Set<string>(RESERVED_ENV_NAMES);

const checkVariables = (
  data: { variables: z.infer<typeof variableRow>[] },
  ctx: z.RefinementCtx
) => {
  const seen = new Set<string>();
  for (const [i, row] of data.variables.entries()) {
    const path = ['variables', i, 'envVariable'];
    if (RESERVED.has(row.envVariable)) {
      ctx.addIssue({
        code: 'custom',
        message: 'Reserved name — choose another.',
        path
      });
    } else if (seen.has(row.envVariable)) {
      ctx.addIssue({
        code: 'custom',
        message: 'Duplicate env variable name.',
        path
      });
    }
    seen.add(row.envVariable);
  }
};

const checkRuntime = (
  data: { startupCommands: z.infer<typeof startupCommandRow>[] },
  ctx: z.RefinementCtx
) => {
  const labels = new Set<string>();
  const commands = new Set<string>();
  for (const [i, row] of data.startupCommands.entries()) {
    if (labels.has(row.label)) {
      ctx.addIssue({
        code: 'custom',
        message: 'Duplicate label.',
        path: ['startupCommands', i, 'label']
      });
    }
    if (commands.has(row.command)) {
      ctx.addIssue({
        code: 'custom',
        message: 'Startup commands must be distinct.',
        path: ['startupCommands', i, 'command']
      });
    }
    labels.add(row.label);
    commands.add(row.command);
  }
};

const checkConfig = (
  data: { configFiles: z.infer<typeof configFileRow>[] },
  ctx: z.RefinementCtx
) => {
  const paths = new Set<string>();
  for (const [i, file] of data.configFiles.entries()) {
    if (paths.has(file.path)) {
      ctx.addIssue({
        code: 'custom',
        message: 'Duplicate file path.',
        path: ['configFiles', i, 'path']
      });
    }
    paths.add(file.path);
    for (const [j, entry] of file.find.entries()) {
      const path = ['configFiles', i, 'find', j, 'value'];
      const [mode] = entry.mode;
      if (
        mode === 'number' &&
        (entry.value.trim() === '' || !Number.isFinite(Number(entry.value)))
      ) {
        ctx.addIssue({ code: 'custom', message: 'Must be a number.', path });
      }
      if (
        mode === 'boolean' &&
        entry.value !== 'true' &&
        entry.value !== 'false'
      ) {
        ctx.addIssue({
          code: 'custom',
          message: 'Must be true or false.',
          path
        });
      }
    }
  }
};

const generalStageObject = z.object({
  author: z.email('Author must be a valid email address').default(''),
  customFeatures: z.array(z.string()).default([]),
  description: z.string().default(''),
  featureEula: z.boolean().default(false),
  featureGslToken: z.boolean().default(false),
  featureJavaVersion: z.boolean().default(false),
  featurePidLimit: z.boolean().default(false),
  featureSteamDiskSpace: z.boolean().default(false),
  icon: z.string().default(''),
  name: z
    .string()
    .min(1, 'Egg name is required')
    .max(255, 'Egg name must be 255 characters or fewer')
    .default(''),
  tags: z.array(z.string()).default([]),
  updateUrl: z.string().default('')
});

const variablesStageObject = z.object({
  variables: z.array(variableRow).default([])
});

const installStageObject = z.object({
  scriptsContainer: z
    .string()
    .min(1, 'Install container image is required')
    .default(''),
  scriptsEntrypoint: z
    .string()
    .min(1, 'Entrypoint shell is required')
    .default('bash'),
  scriptsScript: z.string().default('')
});

const runtimeStageObject = z.object({
  configStartupDone: z
    .array(z.string().min(1, 'Must not be empty'))
    .min(1, 'At least one boot-detection string is required')
    .default([]),
  configStop: z.string().min(1, 'Stop command is required').default(''),
  configStripAnsi: z.boolean().default(false),
  configUserInteraction: z.array(z.string()).default([]),
  dockerImages: z
    .array(dockerImageRow)
    .min(1, 'At least one docker image is required')
    .default([{ image: '', label: '' }]),
  startupCommands: z
    .array(startupCommandRow)
    .min(1, 'At least one startup command is required')
    .default([{ command: '', label: 'Default' }])
});

const configStageObject = z.object({
  configFiles: z.array(configFileRow).default([]),
  configLogsCustom: z.boolean().default(false),
  configLogsLocation: z.string().default(''),
  fileDenylist: z.array(z.string()).default([])
});

// Per-stage schemas — one per wizard Stage's own BaseForm.
export const generalStage = generalStageObject;
export const variablesStage = variablesStageObject.superRefine(checkVariables);
export const installStage = installStageObject;
export const runtimeStage = runtimeStageObject.superRefine(checkRuntime);
export const configStage = configStageObject.superRefine(checkConfig);

/** Form keys owned by each wizard stage, in stage order. */
export const STAGE_KEYS = [
  generalStageObject,
  variablesStageObject,
  installStageObject,
  runtimeStageObject,
  configStageObject
].map((s) => Object.keys(s.shape));

// Combined schema for the detail/edit page's single BaseForm.
export const eggSettings = z
  .object({
    ...generalStageObject.shape,
    ...variablesStageObject.shape,
    ...installStageObject.shape,
    ...runtimeStageObject.shape,
    ...configStageObject.shape
  })
  .superRefine((data, ctx) => {
    checkVariables(data, ctx);
    checkRuntime(data, ctx);
    checkConfig(data, ctx);
  });

export type EggForm = FormValue<typeof eggSettings>;

/**
 * Hands the detail page's full-egg form to a stage's *Fields component.
 * Sound at runtime (EggForm has every stage key); TS rejects it directly only
 * because SuperForm is invariant through its onUpdate callback.
 */
export const stageForm = <T extends FormRecord>(form: SuperForm<EggForm>) =>
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- deliberate widening, see above
  form as unknown as SuperForm<T>;
type FindEntry = z.infer<typeof findEntryRow>;

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

// Pelican's EggImporterService placeholder upgrades for config.files.
// The memory one is anchored so an already-upgraded `memory_limit` is left alone.
const PLACEHOLDER_UPGRADES: [RegExp, string][] = [
  [/server\.build\.env\.SERVER_IP/g, 'server.allocations.default.ip'],
  [/server\.build\.default\.ip/g, 'server.allocations.default.ip'],
  [/server\.build\.env\.SERVER_PORT/g, 'server.allocations.default.port'],
  [/server\.build\.default\.port/g, 'server.allocations.default.port'],
  [/server\.build\.env\.SERVER_MEMORY/g, 'server.build.memory_limit'],
  [/server\.build\.memory(?!\w)/g, 'server.build.memory_limit'],
  [/server\.build\.env\./g, 'server.environment.'],
  [/server\.build\.environment\./g, 'server.environment.']
];

const parseJson = <T>(value: string | T | null | undefined): T | undefined => {
  if (typeof value !== 'string') {
    return value ?? undefined;
  }
  try {
    const parsed: T = JSON.parse(value);
    return parsed;
  } catch {
    return undefined;
  }
};

type LegacyEgg = Exclude<AnyEgg, PelicanEggV3>;

const isEggObject = (raw: unknown): raw is AnyEgg =>
  typeof raw === 'object' && raw !== null;

/** `version` is derived from `egg` by `upgradeEgg`. */
const isPelicanV3 = (egg: AnyEgg, version: unknown): egg is PelicanEggV3 =>
  version === 'PLCN_v3';

/** PTDL_v1 has `image` / `images[]` instead of `docker_images`. */
const legacyDockerImages = (egg: LegacyEgg): Record<string, string> => {
  if ('docker_images' in egg && egg.docker_images) {
    return egg.docker_images;
  }
  const images = 'images' in egg ? egg.images : undefined;
  const image = 'image' in egg ? egg.image : undefined;
  const list = images?.length ? images : [image ?? 'nil'];
  return Object.fromEntries(list.map((img) => [img, img]));
};

const upgradeFiles = (files: EggConfigRaw['files'] | undefined) => {
  if (files === null || files === undefined) {
    return null;
  }
  let text = typeof files === 'string' ? files : JSON.stringify(files);
  for (const [from, to] of PLACEHOLDER_UPGRADES) {
    text = text.replace(from, to);
  }
  return text;
};

// oxlint-disable-next-line eslint/sort-keys -- PLCN_v3 variable key order
const upgradeVariable = (v: LegacyVariable, i: number): PelicanVariable => ({
  name: v.name,
  description: v.description,
  env_variable: v.env_variable,
  default_value: v.default_value,
  user_viewable: v.user_viewable,
  user_editable: v.user_editable,
  rules:
    typeof v.rules === 'string'
      ? v.rules.split('|')
      : ((v.rules as string[]) ?? []),
  sort: i + 1
});

/**
 * The `meta.version` of an egg file. An API `EggInfo` (PLCN_v3 without
 * `meta.version`, recognized by its `startup_commands` map) counts as PLCN_v3.
 * Throws on anything else.
 */
export const eggVersion = (raw: unknown): EggVersion => {
  if (!isEggObject(raw)) {
    throw new Error('The file format is not recognized.');
  }
  const version =
    raw.meta?.version ??
    ('startup_commands' in raw && typeof raw.startup_commands === 'object'
      ? 'PLCN_v3'
      : undefined);
  if (!EGG_VERSIONS.includes(version)) {
    throw new Error('The file format is not recognized.');
  }
  return version;
};

/** Upgrades any supported egg version to PLCN_v3, like Pelican's importer. */
export const upgradeEgg = (raw: unknown): PelicanEggV3 => {
  const version = eggVersion(raw);
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- eggVersion only passes egg objects
  const egg = raw as AnyEgg;
  if (isPelicanV3(egg, version)) {
    return egg;
  }

  // oxlint-disable-next-line eslint/sort-keys -- PLCN_v3 export key order
  return {
    _comment: PELICAN_COMMENT,
    meta: { update_url: egg.meta.update_url ?? null, version: 'PLCN_v3' },
    exported_at: egg.exported_at,
    name: egg.name,
    author: egg.author,
    uuid: '',
    description: egg.description ?? null,
    icon: null,
    tags: [],
    features: egg.features ?? null,
    docker_images: legacyDockerImages(egg),
    file_denylist: egg.file_denylist ?? [],
    startup_commands: { Default: egg.startup },
    config: { ...egg.config, files: upgradeFiles(egg.config?.files) },
    scripts: egg.scripts,
    variables: (egg.variables ?? []).map(upgradeVariable)
  };
};

const findMode = (val: EggConfigFindScalar): FindEntry['mode'][number] => {
  if (typeof val === 'number') {
    return 'number';
  }
  if (typeof val === 'boolean') {
    return 'boolean';
  }
  return 'string';
};

const toFindEntry = (key: string, val: EggConfigFindValue): FindEntry => {
  if (val !== null && typeof val === 'object') {
    return {
      key,
      map: Object.entries(val).map(([match, replace]) => ({
        match,
        // Ponytail: map replacements are edited as strings; typed replacements lose their type.
        replace: replace === null ? '' : String(replace)
      })),
      mode: ['map'],
      value: ''
    };
  }
  return {
    key,
    map: [],
    mode: [findMode(val)],
    value: val === null ? '' : String(val)
  };
};

const doneList = (done: EggConfigStartup['done'] | undefined): string[] => {
  if (Array.isArray(done)) {
    return done;
  }
  return done ? [done] : [];
};

const formatConfig = (config: EggConfigRaw | undefined) => {
  const files = parseJson<Record<string, EggConfigFile>>(config?.files) ?? {};
  const startup = parseJson<EggConfigStartup>(config?.startup) ?? {
    done: []
  };
  const logs = parseJson<EggConfigLogs>(config?.logs) ?? {};
  return {
    configFiles: Object.entries(files).map(([path, file]) => ({
      find: Object.entries(file?.find ?? {}).map(([k, v]) => toFindEntry(k, v)),
      parser: [file?.parser ?? 'file'],
      path
    })),
    configLogsCustom: Boolean(logs.custom),
    configLogsLocation: logs.location ?? '',
    configStartupDone: doneList(startup.done),
    configStop: config?.stop ?? '',
    configStripAnsi: Boolean(startup.strip_ansi),
    configUserInteraction: startup.userInteraction ?? []
  };
};

const formatScripts = (scripts: PelicanEggV3['scripts'] | undefined) => ({
  scriptsContainer: scripts?.installation?.container ?? '',
  scriptsEntrypoint: scripts?.installation?.entrypoint ?? 'bash',
  scriptsScript: scripts?.installation?.script ?? ''
});

const formatVariables = (variables: PelicanVariable[] | undefined) =>
  (variables ?? [])
    .map((v, i) => ({ i, v }))
    .toSorted(
      (a, b) => (a.v.sort ?? Infinity) - (b.v.sort ?? Infinity) || a.i - b.i
    )
    .map(({ v }) => ({
      defaultValue: v.default_value ?? '',
      description: v.description ?? '',
      envVariable: v.env_variable ?? '',
      name: v.name ?? '',
      rules: v.rules ?? [],
      userEditable: v.user_editable ?? false,
      userViewable: v.user_viewable ?? false
    }));

/** Any egg file (parsed JSON/YAML) or API `EggInfo` -> form value. Throws on unknown versions. */
export const formatData = (raw: unknown): EggForm => {
  const egg = upgradeEgg(raw);
  const features = egg.features ?? [];
  const knownKeys = new Set<string>(KNOWN_FEATURES.map((f) => f.key));
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- fromEntries over KNOWN_FEATURES yields exactly these keys
  const featureFlags = Object.fromEntries(
    KNOWN_FEATURES.map((f) => [f.formKey, features.includes(f.key)])
  ) as Record<(typeof KNOWN_FEATURES)[number]['formKey'], boolean>;

  const dockerImages = Object.entries(egg.docker_images ?? {}).map(
    ([label, image]) => ({
      image,
      label
    })
  );
  const startupCommands = Object.entries(egg.startup_commands ?? {}).map(
    ([label, command]) => ({
      command,
      label
    })
  );
  const image =
    typeof egg.image === 'string' && egg.image.startsWith('data:')
      ? egg.image
      : '';

  return {
    ...featureFlags,
    ...formatConfig(egg.config),
    ...formatScripts(egg.scripts),
    author: egg.author ?? '',
    customFeatures: features.filter((f) => !knownKeys.has(f)),
    description: egg.description ?? '',
    dockerImages: dockerImages.length
      ? dockerImages
      : [{ image: '', label: '' }],
    fileDenylist: egg.file_denylist ?? [],
    icon: egg.icon ?? image,
    name: egg.name ?? '',
    startupCommands: startupCommands.length
      ? startupCommands
      : [{ command: '', label: 'Default' }],
    tags: egg.tags ?? [],
    updateUrl: egg.meta?.update_url ?? '',
    variables: formatVariables(egg.variables)
  };
};

/** An imported egg file: its form value plus the version it was upgraded from. */
export interface EggImport {
  form: EggForm;
  sourceVersion: EggVersion;
}

/** Parsed egg file -> form value and source version. Throws on unknown versions. */
export const importEgg = (raw: unknown): EggImport => ({
  form: formatData(raw),
  sourceVersion: eggVersion(raw)
});

const parseEggJson = (text: string): unknown => {
  try {
    return JSON.parse(text);
  } catch {
    throw new Error('Invalid egg file');
  }
};

/** Reads and imports an egg JSON file. Throws with a user-facing message. */
export const readEggFile = async (file: File): Promise<EggImport> =>
  importEgg(parseEggJson(await file.text()));

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

const fromFindEntry = (entry: FindEntry): EggConfigFindValue => {
  switch (entry.mode[0]) {
    case 'number': {
      return Number(entry.value);
    }
    case 'boolean': {
      return entry.value === 'true';
    }
    case 'map': {
      return Object.fromEntries(
        entry.map.map((r): [string, EggConfigFindScalar] => [
          r.match,
          r.replace
        ])
      );
    }
    default: {
      return entry.value;
    }
  }
};

// Panel exports store these as pretty-printed (4 space) JSON strings.
const stringify = (value: object) => JSON.stringify(value, undefined, 4);

const buildConfig = (data: EggForm): PelicanEggV3['config'] => {
  const files = Object.fromEntries(
    data.configFiles.map((file): [string, EggConfigFile] => [
      file.path,
      // oxlint-disable-next-line eslint/sort-keys -- panel export key order
      {
        parser: file.parser[0],
        find: Object.fromEntries(
          file.find.map((e) => [e.key, fromFindEntry(e)])
        )
      }
    ])
  );
  const done = data.configStartupDone;
  const startup: EggConfigStartup = {
    done: done.length === 1 ? done[0] : done
  };
  if (data.configUserInteraction.length) {
    startup.userInteraction = data.configUserInteraction;
  }
  if (data.configStripAnsi) {
    startup.strip_ansi = true;
  }
  const logs: EggConfigLogs =
    data.configLogsCustom || data.configLogsLocation
      ? { custom: data.configLogsCustom, location: data.configLogsLocation }
      : {};

  // oxlint-disable-next-line eslint/sort-keys -- panel export key order
  return {
    files: stringify(files),
    startup: stringify(startup),
    logs: stringify(logs),
    stop: data.configStop || null
  };
};

export type ExportVersion = 'PLCN_v3' | 'PTDL_v2';

/** Form value -> egg file in Pelican (default) or Pterodactyl format, keys in panel export order. */
export function reformatData(
  data: EggForm,
  version?: 'PLCN_v3',
  uuid?: string
): PelicanEggV3;
export function reformatData(
  data: EggForm,
  version: 'PTDL_v2',
  uuid?: string
): PterodactylEggV2;
export function reformatData(
  data: EggForm,
  version: ExportVersion = 'PLCN_v3',
  uuid: string = crypto.randomUUID()
): PelicanEggV3 | PterodactylEggV2 {
  const features = [
    ...KNOWN_FEATURES.filter((f) => data[f.formKey]).map((f) => f.key),
    ...data.customFeatures
  ];
  const dockerImages = Object.fromEntries(
    data.dockerImages.map((r) => [r.label, r.image])
  );
  const updateUrl = data.updateUrl.trim() || null;
  const exportedAt = new Date().toISOString();
  const description = data.description || null;
  const config = buildConfig(data);
  const scripts = {
    // oxlint-disable-next-line eslint/sort-keys -- panel export key order
    installation: {
      script: data.scriptsScript || null,
      container: data.scriptsContainer,
      entrypoint: data.scriptsEntrypoint
    }
  };

  if (version === 'PTDL_v2') {
    // oxlint-disable-next-line eslint/sort-keys -- PTDL_v2 export key order
    return {
      _comment: PTERODACTYL_COMMENT,
      // oxlint-disable-next-line eslint/sort-keys -- PTDL_v2 export key order
      meta: { version: 'PTDL_v2', update_url: updateUrl },
      exported_at: exportedAt,
      name: data.name,
      author: data.author,
      description,
      features,
      docker_images: dockerImages,
      file_denylist: data.fileDenylist,
      startup: data.startupCommands[0]?.command ?? '',
      config,
      scripts,
      // oxlint-disable-next-line eslint/sort-keys -- PTDL_v2 variable key order
      variables: data.variables.map((v) => ({
        name: v.name,
        description: v.description,
        env_variable: v.envVariable,
        default_value: v.defaultValue,
        user_viewable: v.userViewable,
        user_editable: v.userEditable,
        rules: v.rules.join('|'),
        field_type: 'text'
      }))
    };
  }

  // oxlint-disable-next-line eslint/sort-keys -- PLCN_v3 export key order
  return {
    _comment: PELICAN_COMMENT,
    // oxlint-disable-next-line eslint/sort-keys -- PLCN_v3 export key order
    meta: { version: 'PLCN_v3', update_url: updateUrl },
    exported_at: exportedAt,
    name: data.name,
    author: data.author,
    uuid,
    description,
    icon: data.icon || null,
    tags: data.tags,
    features,
    docker_images: dockerImages,
    file_denylist: data.fileDenylist,
    startup_commands: Object.fromEntries(
      data.startupCommands.map((r) => [r.label, r.command])
    ),
    config,
    scripts,
    // oxlint-disable-next-line eslint/sort-keys -- PLCN_v3 variable key order
    variables: data.variables.map((v, i) => ({
      name: v.name,
      description: v.description,
      env_variable: v.envVariable,
      default_value: v.defaultValue,
      user_viewable: v.userViewable,
      user_editable: v.userEditable,
      rules: v.rules,
      sort: i + 1
    }))
  };
}

/** Lowercase dash slug for download file names. */
export const slugify = (name: string) =>
  name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '') || 'egg';

/** Form value -> API `EggBody` (PLCN_v3 without `_comment`, `exported_at`, `uuid`). */
export const toEggBody = (data: EggForm): EggBody => {
  const {
    _comment: _c,
    exported_at: _e,
    uuid: _u,
    image: _i,
    meta,
    ...body
  } = reformatData(data, 'PLCN_v3', '');
  return { ...body, meta: { update_url: meta.update_url ?? null } };
};

/** Server error body (a message string on 400) or `fallback`. */
export const apiErrorMessage = (error: unknown, fallback: string) =>
  typeof error === 'string' && error.trim() ? error : fallback;
