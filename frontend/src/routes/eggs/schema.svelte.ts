import type { FormValue } from '@profidev/pleiades/components/form/types';
import { z } from 'zod';

// Mirrors egg.d.ts's EggVariable.RESERVED_ENV_NAMES / EggConfigFile.parser / EggFeature.
export const RESERVED_ENV_NAMES = [
  'SERVER_MEMORY',
  'SERVER_IP',
  'SERVER_PORT',
  'ENV',
  'HOME',
  'USER',
  'STARTUP',
  'SERVER_UUID',
  'UUID'
];

export const PARSER_VALUES = [
  'file',
  'yaml',
  'properties',
  'ini',
  'json',
  'xml'
] as const;
export const PARSER_OPTIONS = PARSER_VALUES.map((v) => ({ label: v, value: v }));

export const FIELD_TYPE_OPTIONS = [
  { label: 'text', value: 'text' },
  { label: 'password', value: 'password' }
];

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
      'Reveals a FastDL settings section in the admin/egg configuration UI (Source engine games).',
    formKey: 'featureFastdl',
    key: 'fastdl',
    label: 'FastDL'
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
    formKey: 'featureSteamcmdDiskSpace',
    key: 'steamcmd_disk_space',
    label: 'SteamCMD disk space'
  }
] as const;

export const dockerImageRow = z.object({
  image: z.string().min(1, 'Image reference is required').default(''),
  label: z.string().min(1, 'Label is required').default('')
});

export const findEntryRow = z.object({
  isRegex: z.boolean().default(false),
  key: z.string().min(1, 'Key is required').default(''),
  regexPattern: z.string().default(''),
  regexReplacement: z.string().default(''),
  value: z.string().default('')
});

export const configFileRow = z.object({
  find: z.array(findEntryRow).default([]),
  parser: z.array(z.enum(PARSER_VALUES)).min(1).max(1).default(['file']),
  path: z.string().min(1, 'File path is required').default('')
});

export const variableRow = z.object({
  defaultValue: z.string().default(''),
  description: z.string().default(''),
  envVariable: z
    .string()
    .min(1, 'Env variable is required')
    .regex(/^[A-Za-z0-9_]+$/, 'Letters, numbers and underscore only')
    .default(''),
  fieldType: z.array(z.enum(['text', 'password'])).min(1).max(1).default(['text']),
  name: z
    .string()
    .min(1, 'Name is required')
    .max(255, 'Max 255 characters')
    .default(''),
  rules: z.string().min(1, 'At least one validation rule is required').default(''),
  userEditable: z.boolean().default(true),
  userViewable: z.boolean().default(true)
});

const checkVariables = (
  data: { variables: z.infer<typeof variableRow>[] },
  ctx: z.RefinementCtx
) => {
  const seen = new Set<string>();
  data.variables.forEach((row, i) => {
    if (RESERVED_ENV_NAMES.includes(row.envVariable.toUpperCase())) {
      ctx.addIssue({
        code: 'custom',
        message: 'Reserved name — choose another.',
        path: ['variables', i, 'envVariable']
      });
    } else if (seen.has(row.envVariable)) {
      ctx.addIssue({
        code: 'custom',
        message: 'Duplicate env variable name.',
        path: ['variables', i, 'envVariable']
      });
    }
    seen.add(row.envVariable);
  });
};

const generalStageObject = z.object({
  author: z.email('Author must be a valid email address').default(''),
  customFeatures: z.array(z.string()).default([]),
  description: z.string().default(''),
  featureEula: z.boolean().default(false),
  featureFastdl: z.boolean().default(false),
  featureGslToken: z.boolean().default(false),
  featureJavaVersion: z.boolean().default(false),
  featurePidLimit: z.boolean().default(false),
  featureSteamcmdDiskSpace: z.boolean().default(false),
  name: z
    .string()
    .min(1, 'Egg name is required')
    .max(191, 'Egg name must be 191 characters or fewer')
    .default(''),
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
    .string()
    .min(1, 'At least one boot-detection string is required')
    .default(''),
  configStop: z.string().min(1, 'Stop command is required').default(''),
  configStripAnsi: z.boolean().default(false),
  configUserInteraction: z.array(z.string()).default([]),
  dockerImages: z
    .array(dockerImageRow)
    .min(1, 'At least one docker image is required')
    .default([{ image: '', label: '' }]),
  startup: z.string().min(1, 'Startup command is required').default('')
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
export const runtimeStage = runtimeStageObject;
export const configStage = configStageObject;

// Combined schema for the detail/edit page's single BaseForm.
export const eggSettings = z
  .object({
    ...generalStageObject.shape,
    ...variablesStageObject.shape,
    ...installStageObject.shape,
    ...runtimeStageObject.shape,
    ...configStageObject.shape
  })
  .superRefine(checkVariables);

const coerceFindValue = (raw: string): string | number | boolean => {
  if (raw === 'true') return true;
  if (raw === 'false') return false;
  if (raw.trim() !== '' && /^-?\d+(\.\d+)?$/.test(raw.trim())) return Number(raw);
  return raw;
};

export interface EggJson {
  _comment: string;
  author: string;
  config: {
    files: Record<string, { find: Record<string, unknown>; parser: string }>;
    logs: { custom: boolean; location: string };
    startup: {
      done: string | string[];
      strip_ansi: boolean;
      userInteraction: string[];
    };
    stop: string;
  };
  description: string | null;
  docker_images: Record<string, string>;
  exported_at: string;
  features: string[] | null;
  file_denylist: string[];
  meta: { update_url: string | null; version: 'PTDL_v2' };
  name: string;
  scripts: {
    installation: { container: string; entrypoint: string; script: string | null };
  };
  startup: string;
  variables: {
    default_value: string;
    description: string;
    env_variable: string;
    field_type: string;
    name: string;
    rules: string;
    user_editable: boolean;
    user_viewable: boolean;
  }[];
}

// Ports buildExportJson() from the "Egg Editor - Wizard" design mock 1:1 onto our field names.
export const reformatData = (data: FormValue<typeof eggSettings>): EggJson => {
  const dockerImages: Record<string, string> = {};
  data.dockerImages.forEach((row) => {
    if (row.label.trim()) dockerImages[row.label.trim()] = row.image.trim();
  });

  const files: EggJson['config']['files'] = {};
  data.configFiles.forEach((file) => {
    if (!file.path.trim()) return;
    const find: Record<string, unknown> = {};
    file.find.forEach((entry) => {
      if (!entry.key.trim()) return;
      if (entry.isRegex) {
        find[entry.key] = {
          [`regex:${entry.regexPattern}`]: entry.regexReplacement
        };
      } else {
        find[entry.key] = coerceFindValue(entry.value);
      }
    });
    files[file.path.trim()] = { find, parser: file.parser[0] };
  });

  const doneParts = data.configStartupDone
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);
  const features = [
    ...KNOWN_FEATURES.filter((f) => data[f.formKey]).map((f) => f.key),
    ...data.customFeatures.filter(Boolean)
  ];

  return {
    _comment: 'DO NOT EDIT: FILE GENERATED AUTOMATICALLY BY EGG EDITOR',
    author: data.author,
    config: {
      files,
      logs: { custom: data.configLogsCustom, location: data.configLogsLocation || '' },
      startup: {
        done: doneParts.length > 1 ? doneParts : doneParts[0] || '',
        strip_ansi: data.configStripAnsi,
        userInteraction: data.configUserInteraction.filter(Boolean)
      },
      stop: data.configStop
    },
    description: data.description || null,
    docker_images: dockerImages,
    exported_at: new Date().toISOString(),
    features: features.length ? features : null,
    file_denylist: data.fileDenylist.filter(Boolean),
    meta: { update_url: data.updateUrl.trim() || null, version: 'PTDL_v2' },
    name: data.name,
    scripts: {
      installation: {
        container: data.scriptsContainer,
        entrypoint: data.scriptsEntrypoint,
        script: data.scriptsScript || null
      }
    },
    startup: data.startup,
    variables: data.variables.map((v) => ({
      default_value: v.defaultValue || '',
      description: v.description || '',
      env_variable: v.envVariable,
      field_type: v.fieldType[0] ?? 'text',
      name: v.name,
      rules: v.rules,
      user_editable: v.userEditable,
      user_viewable: v.userViewable
    }))
  };
};

// Ports parseEggToForm() from the design mock — accepts a raw egg JSON object
// (config.files/startup/logs may still be JSON-encoded strings, per EggConfigV2Raw).
export const formatData = (egg: any): FormValue<typeof eggSettings> => {
  const config = egg.config ?? {};
  let files = config.files;
  let startup = config.startup;
  let logs = config.logs;
  if (typeof files === 'string') {
    try {
      files = JSON.parse(files);
    } catch {
      files = {};
    }
  }
  if (typeof startup === 'string') {
    try {
      startup = JSON.parse(startup);
    } catch {
      startup = { done: '' };
    }
  }
  if (typeof logs === 'string') {
    try {
      logs = JSON.parse(logs);
    } catch {
      logs = { custom: false, location: '' };
    }
  }

  const configFiles = Object.entries(files ?? {}).map(([path, fileObj]: [string, any]) => ({
    find: Object.entries(fileObj?.find ?? {}).map(([key, val]: [string, any]) => {
      if (val && typeof val === 'object' && !Array.isArray(val)) {
        const [rk, rv] = Object.entries(val)[0] ?? ['regex:', ''];
        return {
          isRegex: true,
          key,
          regexPattern: String(rk).replace(/^regex:/, ''),
          regexReplacement: String(rv),
          value: ''
        };
      }
      return { isRegex: false, key, regexPattern: '', regexReplacement: '', value: String(val) };
    }),
    parser: [fileObj?.parser || 'file'],
    path
  }));

  const knownKeys = new Set<string>(KNOWN_FEATURES.map((f) => f.key));
  const allFeatures: string[] = egg.features ?? [];
  const featureFlags = Object.fromEntries(
    KNOWN_FEATURES.map((f) => [f.formKey, allFeatures.includes(f.key)])
  );
  const customFeatures = allFeatures.filter((f) => !knownKeys.has(f));

  const dockerImages = Object.entries(egg.docker_images ?? {}).map(
    ([label, image]) => ({ image: String(image), label })
  );

  const done = startup?.done;
  const configStartupDone = Array.isArray(done) ? done.join(', ') : (done ?? '');

  return {
    ...featureFlags,
    author: egg.author ?? '',
    configFiles,
    configLogsCustom: !!logs?.custom,
    configLogsLocation: logs?.location ?? '',
    configStartupDone,
    configStop: config.stop ?? '',
    configStripAnsi: !!startup?.strip_ansi,
    configUserInteraction: startup?.userInteraction ?? [],
    customFeatures,
    description: egg.description ?? '',
    dockerImages: dockerImages.length ? dockerImages : [{ image: '', label: '' }],
    name: egg.name ?? '',
    scriptsContainer: egg.scripts?.installation?.container ?? '',
    scriptsEntrypoint: egg.scripts?.installation?.entrypoint ?? 'bash',
    scriptsScript: egg.scripts?.installation?.script ?? '',
    startup: egg.startup ?? '',
    updateUrl: egg.meta?.update_url ?? '',
    variables: (egg.variables ?? []).map((v: any) => ({
      defaultValue: v.default_value ?? '',
      description: v.description ?? '',
      envVariable: v.env_variable ?? '',
      fieldType: [v.field_type || 'text'],
      name: v.name ?? '',
      rules: v.rules ?? '',
      userEditable: v.user_editable ?? true,
      userViewable: v.user_viewable ?? true
    }))
  } as FormValue<typeof eggSettings>;
};
