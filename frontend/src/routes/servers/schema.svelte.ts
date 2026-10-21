/* oxlint-disable unicorn/no-null -- the API uses null for "no owner" */
import type {
  FormRecord,
  FormValue,
  SuperForm
} from '@profidev/pleiades/components/form/types';
import { z } from 'zod';
import type {
  CreateServer,
  EggVariable,
  GrantsBody,
  GrantsInfo,
  Limits,
  OptionEgg,
  ServerInfo,
  ServerOptions,
  UpdateServer
} from '$lib/client';
import { validateVariable } from './rules';

/** Form sections in wizard / tab order. */
export const SECTIONS = [
  'general',
  'startup',
  'variables',
  'resources',
  'permissions'
] as const;
export type Section = (typeof SECTIONS)[number];

export const SECTION_TITLES: Record<Section, string> = {
  general: 'General',
  permissions: 'Permissions',
  resources: 'Resources',
  startup: 'Startup',
  variables: 'Variables'
};

export const SECTION_DESCRIPTIONS: Record<Section, string> = {
  general: 'Name, owner, node, egg and allocations.',
  permissions: 'Users and groups that may view or manage this server.',
  resources: 'CPU, memory, storage and backup limits.',
  startup: 'Install behavior, startup command and Docker image.',
  variables: "Values for the egg's variables. They replace the egg defaults."
};

/** Shown when a limit switch is turned off. */
export const LIMIT_DEFAULTS = { cpu: 1000, disk: 10_240, memory: 4096 };
export const DEFAULT_PORT = 25_565;

const isIp = (v: string) =>
  z.ipv4().safeParse(v).success || z.ipv6().safeParse(v).success;

export const allocationRow = z.object({
  ip: z
    .string()
    .trim()
    .refine(isIp, 'Enter a valid IPv4 or IPv6 address.')
    .default(''),
  port: z
    .number({ error: 'Port is required.' })
    .int('Port must be a whole number.')
    .min(1024, 'Port must be between 1024 and 65535.')
    .max(65_535, 'Port must be between 1024 and 65535.')
    .default(DEFAULT_PORT)
});

/** One egg variable with its metadata, so the section validates on its own. */
export const variableRow = z.object({
  defaultValue: z.string().default(''),
  description: z.string().default(''),
  /** False when the caller may only view it; it is then not sent. */
  editable: z.boolean().default(true),
  env: z.string(),
  name: z.string(),
  rules: z.array(z.string()).default([]),
  value: z.string().default('')
});

export const GRANT_ACCESS = ['read', 'write'] as const;

/** One user or group grant; `uuid` holds the FormSelect's single value. */
const grantRow = (kind: 'user' | 'group') =>
  z.object({
    access: z
      .array(z.enum(GRANT_ACCESS))
      .min(1, 'Select an access level.')
      .default(['read']),
    uuid: z.array(z.string()).min(1, `Select a ${kind}.`).default([])
  });

const selectOne = (message: string) =>
  z.array(z.string()).min(1, message).max(1).default([]);

/** Matches the backend; longer would block a stop for ages. */
export const MAX_STOP_TIMEOUT = 3600;

const count = (label: string, unit: string) =>
  z
    .number({ error: `${label} is required.` })
    .int(`${label} must be a whole number.`)
    .min(0, `${label} must be 0 or more ${unit}.`);

/** Only checked while revealed (its unlimited switch is off), see checkLimits. */
const limit = z.number().nullable();

const generalObject = z.object({
  allocations: z
    .array(allocationRow)
    .min(1, 'Add at least one allocation.')
    .default([{ ip: '', port: DEFAULT_PORT }]),
  egg: selectOne('Egg is required.'),
  name: z
    .string()
    .trim()
    .min(1, 'Name is required.')
    .max(191, 'Name must be 191 characters or fewer.')
    .default(''),
  node: selectOne('Node is required.'),
  /** Optional on edit: the backend keeps servers whose owner was deleted (owner null). */
  owner: z.array(z.string()).max(1).default([])
});

const startupObject = z.object({
  image: z.string().trim().min(1, 'Image is required.').default(''),
  restartOnCrash: z.boolean().default(true),
  /** Create only; off sends `skip_scripts`. */
  runInstall: z.boolean().default(true),
  /** Create only. */
  startOnCompletion: z.boolean().default(true),
  startup: z.string().trim().min(1, 'Startup command is required.').default(''),
  startupOverridden: z.boolean().default(false),
  stopTimeout: z
    .number({ error: 'Stop timeout is required.' })
    .int('Stop timeout must be a whole number.')
    .min(1, 'Stop timeout must be at least 1 second.')
    .max(
      MAX_STOP_TIMEOUT,
      `Stop timeout must be at most ${MAX_STOP_TIMEOUT} seconds.`
    )
    .default(60)
});

const variablesObject = z.object({
  variables: z.array(variableRow).default([])
});

const resourcesObject = z.object({
  backupLimit: count('Backup limit', 'backups').default(0),
  cpu: limit.default(LIMIT_DEFAULTS.cpu),
  cpuUnlimited: z.boolean().default(true),
  disk: limit.default(LIMIT_DEFAULTS.disk),
  diskUnlimited: z.boolean().default(true),
  memory: limit.default(LIMIT_DEFAULTS.memory),
  memoryUnlimited: z.boolean().default(true)
});

const permissionsObject = z.object({
  groupGrants: z.array(grantRow('group')).default([]),
  userGrants: z.array(grantRow('user')).default([])
});

type Ctx = z.RefinementCtx;

/** Trimmed, lowercased; any all-zero IPv6 form (`0:0::0`) becomes `::`. */
const normIp = (ip: string) => {
  const v = ip.trim().toLowerCase();
  return v.includes(':') && /^[0:]+$/.test(v) ? '::' : v;
};

/** `::` covers everything; `0.0.0.0` covers IPv4 (mirrors the backend). */
const covers = (a: string, b: string) =>
  a === '::' || (a === '0.0.0.0' && !b.includes(':'));

const checkAllocations = (data: z.infer<typeof generalObject>, ctx: Ctx) => {
  const rows = data.allocations.map((a) => ({
    ip: normIp(a.ip),
    port: a.port
  }));
  for (const [i, a] of rows.entries()) {
    const earlier = rows.slice(0, i).filter((b) => b.port === a.port);
    let message = 'Overlaps another allocation on the same port.';
    if (earlier.some((b) => b.ip === a.ip)) {
      message = 'Duplicate allocation.';
    } else if (!earlier.some((b) => covers(a.ip, b.ip) || covers(b.ip, a.ip))) {
      continue;
    }
    ctx.addIssue({ code: 'custom', message, path: ['allocations', i, 'port'] });
  }
};

const checkVariables = (data: z.infer<typeof variablesObject>, ctx: Ctx) => {
  for (const [i, v] of data.variables.entries()) {
    const message = v.editable
      ? validateVariable(v.value, v.rules, v.name)
      : undefined;
    if (message) {
      ctx.addIssue({
        code: 'custom',
        message,
        path: ['variables', i, 'value']
      });
    }
  }
};

const LIMIT_CHECKS = [
  ['cpu', 'cpuUnlimited', count('CPU', 'millicores')],
  ['memory', 'memoryUnlimited', count('Memory', 'MiB')],
  ['disk', 'diskUnlimited', count('Storage', 'MiB')]
] as const;

const checkLimits = (data: z.infer<typeof resourcesObject>, ctx: Ctx) => {
  for (const [key, unlimited, schema] of LIMIT_CHECKS) {
    if (data[unlimited]) {
      continue;
    }
    const res = schema.safeParse(data[key]);
    if (!res.success) {
      ctx.addIssue({
        code: 'custom',
        message: res.error.issues[0].message,
        path: [key]
      });
    }
  }
};

const checkGrants = (data: z.infer<typeof permissionsObject>, ctx: Ctx) => {
  for (const key of ['userGrants', 'groupGrants'] as const) {
    const seen = new Set<string>();
    for (const [i, g] of data[key].entries()) {
      const [uuid] = g.uuid;
      if (uuid && seen.has(uuid)) {
        ctx.addIssue({
          code: 'custom',
          message: 'Already granted.',
          path: [key, i, 'uuid']
        });
      }
      seen.add(uuid);
    }
  }
};

// Per-section schemas: one per wizard stage's BaseForm.
export const generalStage = generalObject
  .extend({ owner: selectOne('Owner is required.') })
  .superRefine(checkAllocations);
export const startupStage = startupObject;
export const variablesStage = variablesObject.superRefine(checkVariables);
export const resourcesStage = resourcesObject.superRefine(checkLimits);
export const permissionsStage = permissionsObject.superRefine(checkGrants);

/** Form keys owned by each section. */
export const SECTION_KEYS: Record<Section, string[]> = {
  general: Object.keys(generalObject.shape),
  permissions: Object.keys(permissionsObject.shape),
  resources: Object.keys(resourcesObject.shape),
  startup: Object.keys(startupObject.shape),
  variables: Object.keys(variablesObject.shape)
};

/** Combined schema for the edit page's single BaseForm. */
export const serverSettings = z
  .object({
    ...generalObject.shape,
    ...startupObject.shape,
    ...variablesObject.shape,
    ...resourcesObject.shape,
    ...permissionsObject.shape
  })
  .superRefine((data, ctx) => {
    checkAllocations(data, ctx);
    checkVariables(data, ctx);
    checkLimits(data, ctx);
    checkGrants(data, ctx);
  });

export type ServerForm = FormValue<typeof serverSettings>;
export type VariableRow = z.infer<typeof variableRow>;
export type GrantRow = z.infer<ReturnType<typeof grantRow>>;
type Grants = Pick<ServerForm, 'userGrants' | 'groupGrants'>;
export type FieldsMode = 'create' | 'edit';

/**
 * Hands the edit page's full-server form to a section's *Fields component.
 * Sound at runtime (ServerForm has every section key); TS rejects it directly
 * only because SuperForm is invariant through its onUpdate callback.
 */
export const stageForm = <T extends FormRecord>(form: SuperForm<ServerForm>) =>
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- deliberate widening, see above
  form as unknown as SuperForm<T>;

// ---------------------------------------------------------------------------
// Egg defaults
// ---------------------------------------------------------------------------

export const findEgg = (options: ServerOptions | undefined, uuid?: string) =>
  uuid ? options?.eggs.find((e) => e.uuid === uuid) : undefined;

/**
 * Select options built from a server's own references, for callers that may
 * not load `GET /servers/options`: the selects then show the current values.
 */
export const optionsFromInfo = (info: ServerInfo): ServerOptions => ({
  eggs: [
    {
      docker_images: {},
      name: info.egg.name,
      startup_commands: {},
      uuid: info.egg.uuid,
      variables: []
    }
  ],
  groups: [],
  nodes: [info.node],
  users: info.owner ? [info.owner] : []
});

/** The egg's default (first) startup command. */
export const defaultCommand = (egg?: OptionEgg) =>
  Object.values(egg?.startup_commands ?? {})[0] ?? '';

/** Startup command and image of a freshly picked egg. */
export const eggStartup = (egg?: OptionEgg) => ({
  image: Object.values(egg?.docker_images ?? {})[0] ?? '',
  startup: defaultCommand(egg),
  startupOverridden: false
});

const sortVariables = <T extends { sort?: number | null }>(vars: T[]) =>
  vars
    .map((v, i) => ({ i, v }))
    .toSorted((a, b) => (a.v.sort ?? a.i) - (b.v.sort ?? b.i) || a.i - b.i)
    .map(({ v }) => v);

/** Variable rows of a freshly picked egg, holding the egg defaults. */
export const eggVariables = (egg?: OptionEgg): VariableRow[] =>
  sortVariables<EggVariable>(egg?.variables ?? []).map((v) => ({
    defaultValue: v.default_value,
    description: v.description,
    editable: true,
    env: v.env_variable,
    name: v.name,
    rules: v.rules ?? [],
    value: v.default_value
  }));

/** Whether any variable differs from its egg default (changing egg discards it). */
export const variablesEdited = (rows: VariableRow[] = []) =>
  rows.some((r) => r.value !== r.defaultValue);

// ---------------------------------------------------------------------------
// Converters
// ---------------------------------------------------------------------------

const toLimits = (f: ServerForm): Limits => ({
  cpu: f.cpuUnlimited ? 0 : (f.cpu ?? 0),
  disk: f.diskUnlimited ? 0 : (f.disk ?? 0),
  memory: f.memoryUnlimited ? 0 : (f.memory ?? 0)
});

const environment = (rows: VariableRow[]) =>
  Object.fromEntries(
    rows.filter((r) => r.editable).map((r) => [r.env, r.value])
  );

const toGrants = (rows: GrantRow[]) =>
  rows.map((r) => ({ access: r.access[0], uuid: r.uuid[0] }));

const fromGrants = (grants: GrantsInfo['users'] = []): GrantRow[] =>
  grants.map((g) => ({ access: [g.access], uuid: [g.uuid] }));

export const toGrantsBody = (f: Grants): GrantsBody => ({
  groups: toGrants(f.groupGrants),
  users: toGrants(f.userGrants)
});

export const fromGrantsInfo = (grants?: GrantsInfo | null): Grants => ({
  groupGrants: fromGrants(grants?.groups),
  userGrants: fromGrants(grants?.users)
});

const common = (f: ServerForm) => ({
  allocations: f.allocations.map(({ ip, port }) => ({ ip: ip.trim(), port })),
  backup_limit: f.backupLimit,
  egg: f.egg[0],
  environment: environment(f.variables),
  image: f.image.trim(),
  limits: toLimits(f),
  name: f.name.trim(),
  owner: f.owner[0] ?? null,
  restart_on_crash: f.restartOnCrash,
  startup: f.startup.trim(),
  startup_overridden: f.startupOverridden,
  stop_timeout: f.stopTimeout
});

/** Form value -> `POST /servers` body. */
export const toServerBody = (f: ServerForm): CreateServer => ({
  ...common(f),
  node: f.node[0],
  skip_scripts: !f.runInstall,
  start_on_completion: f.startOnCompletion
});

/** Form value -> `PUT /servers/{uuid}` body; grants only when the caller is admin. */
export const toUpdateBody = (
  f: ServerForm,
  withGrants: boolean
): UpdateServer =>
  withGrants ? { ...common(f), grants: toGrantsBody(f) } : common(f);

const fromLimit = (value: number, fallback: number) =>
  value > 0
    ? { unlimited: false, value }
    : { unlimited: true, value: fallback };

/** `GET /servers/{uuid}` -> form value. Install switches keep their defaults. */
export const fromServerInfo = (info: ServerInfo): ServerForm => {
  const cpu = fromLimit(info.limits.cpu, LIMIT_DEFAULTS.cpu);
  const memory = fromLimit(info.limits.memory, LIMIT_DEFAULTS.memory);
  const disk = fromLimit(info.limits.disk, LIMIT_DEFAULTS.disk);
  return {
    ...fromGrantsInfo(info.grants),
    allocations: info.allocations.map(({ ip, port }) => ({ ip, port })),
    backupLimit: info.backup_limit,
    cpu: cpu.value,
    cpuUnlimited: cpu.unlimited,
    disk: disk.value,
    diskUnlimited: disk.unlimited,
    egg: [info.egg.uuid],
    image: info.image,
    memory: memory.value,
    memoryUnlimited: memory.unlimited,
    name: info.name,
    node: [info.node.uuid],
    owner: info.owner ? [info.owner.uuid] : [],
    restartOnCrash: info.restart_on_crash,
    runInstall: true,
    startOnCompletion: info.start_on_completion,
    startup: info.startup,
    startupOverridden: info.startup_overridden,
    stopTimeout: info.stop_timeout,
    variables: info.variables.map((v) => ({
      defaultValue: v.default_value,
      description: v.description,
      editable: v.editable,
      env: v.env_variable,
      name: v.name,
      rules: v.rules,
      value: v.value
    }))
  };
};

// ---------------------------------------------------------------------------
// API field errors
// ---------------------------------------------------------------------------

export interface MappedErrors {
  /** Per section: dotted form path -> message (`allocations._errors` for the list itself). */
  sections: Partial<Record<Section, Record<string, string>>>;
  /** First section in SECTIONS order with an error. */
  first?: Section;
  /** Messages for fields no section shows. */
  other: string[];
}

const DIRECT: Record<string, [Section, string]> = {
  allocations: ['general', 'allocations._errors'],
  backup_limit: ['resources', 'backupLimit'],
  egg: ['general', 'egg'],
  image: ['startup', 'image'],
  'limits.cpu': ['resources', 'cpu'],
  'limits.disk': ['resources', 'disk'],
  'limits.memory': ['resources', 'memory'],
  name: ['general', 'name'],
  node: ['general', 'node'],
  owner: ['general', 'owner'],
  startup: ['startup', 'startup'],
  stop_timeout: ['startup', 'stopTimeout']
};

export const isFieldErrors = (
  body: unknown
): body is { errors: { field: string; message: string }[] } =>
  typeof body === 'object' &&
  body !== null &&
  Array.isArray((body as { errors?: unknown }).errors);

const locate = (
  field: string,
  form: Partial<Pick<ServerForm, 'variables' | 'userGrants' | 'groupGrants'>>
): [Section, string] | undefined => {
  if (Object.hasOwn(DIRECT, field)) {
    return DIRECT[field];
  }
  const alloc = /^allocations\.(?<index>\d+)(?:\.(?<key>ip|port))?$/.exec(
    field
  );
  if (alloc) {
    const { index, key = 'ip' } = alloc.groups!;
    return ['general', `allocations.${index}.${key}`];
  }
  if (field.startsWith('variables.')) {
    const env = field.slice('variables.'.length);
    const i = (form.variables ?? []).findIndex((v) => v.env === env);
    return i === -1 ? undefined : ['variables', `variables.${i}.value`];
  }
  const grant = /^grants\.(?<kind>users|groups)\.(?<index>\d+)/.exec(field);
  if (grant) {
    const { kind, index } = grant.groups!;
    const key = kind === 'users' ? 'userGrants' : 'groupGrants';
    return form[key]?.[Number(index)]
      ? ['permissions', `${key}.${index}.uuid`]
      : undefined;
  }
  return undefined;
};

/**
 * Maps a 400/409 `FieldErrors` body onto form paths and sections.
 * Returns undefined when `body` is not a FieldErrors body.
 * `form` resolves `variables.<ENV>` to a row index and drops `grants.*.N` without a row.
 */
export const mapFieldErrors = (
  body: unknown,
  form: Partial<
    Pick<ServerForm, 'variables' | 'userGrants' | 'groupGrants'>
  > = {}
): MappedErrors | undefined => {
  if (!isFieldErrors(body)) {
    return undefined;
  }
  const mapped: MappedErrors = { other: [], sections: {} };
  for (const { field, message } of body.errors) {
    const hit = locate(field, form);
    if (hit) {
      const [section, path] = hit;
      (mapped.sections[section] ??= {})[path] ??= message;
    } else {
      mapped.other.push(message);
    }
  }
  mapped.first = SECTIONS.find((s) => mapped.sections[s]);
  return mapped;
};

// ---------------------------------------------------------------------------
// Create wizard
// ---------------------------------------------------------------------------

/** Shared `data` of every create-wizard stage. */
export interface WizardData {
  options: ServerOptions;
  /** Merged values of the stages entered so far. */
  values: () => Partial<ServerForm>;
  /** The user picked another egg (after confirming); resets egg-derived stages. */
  oneggchange: (egg: OptionEgg | undefined) => void;
  /** Server errors for a section, cleared once taken. */
  takeErrors: (section: Section) => Record<string, string> | undefined;
}
