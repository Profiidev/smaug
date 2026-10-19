import { afterEach, describe, expect, it, vi } from 'vitest';
import type { z } from 'zod';
import {
  type EggForm,
  FIND_MODE_OPTIONS,
  KNOWN_FEATURES,
  PARSER_OPTIONS,
  STAGE_KEYS,
  apiErrorMessage,
  configStage,
  eggSettings,
  eggVersion,
  formatData,
  generalStage,
  importEgg,
  installStage,
  readEggFile,
  reformatData,
  runtimeStage,
  slugify,
  stageForm,
  toEggBody,
  upgradeEgg,
  variablesStage
} from '$routes/eggs/schema.svelte';
import {
  FEATURES,
  PARSERS,
  PELICAN_COMMENT,
  PTERODACTYL_COMMENT,
  RESERVED_ENV_NAMES
} from '$lib/egg';
import { eggInfo } from '$mocks/e2e/data';

const clone = <T>(v: T): T => structuredClone(v);

/** A valid form value derived from the API fixture. */
const base = (): EggForm => formatData(clone(eggInfo));

/** Issue messages keyed by dotted path. */
const issues = (schema: z.ZodType, value: unknown) => {
  const res = schema.safeParse(value);
  if (res.success) {
    return {};
  }
  const out: Record<string, string[]> = {};
  for (const i of res.error.issues) {
    (out[i.path.join('.')] ??= []).push(i.message);
  }
  return out;
};

const variable = (over: Partial<EggForm['variables'][number]> = {}) => ({
  defaultValue: '',
  description: '',
  envVariable: 'MY_VAR',
  name: 'My Var',
  rules: ['required'],
  userEditable: false,
  userViewable: false,
  ...over
});

const legacyV2 = (version: string) => ({
  _comment: PTERODACTYL_COMMENT,
  author: 'dev@example.com',
  config: {
    files:
      '{"server.properties":{"parser":"properties","find":{"server-ip":"{{server.build.default.ip}}","server-port":"{{server.build.env.SERVER_PORT}}"}}}',
    logs: '{}',
    startup: '{"done":"Done"}',
    stop: 'stop'
  },
  description: null,
  docker_images: { 'Java 17': 'ghcr.io/yolks:java_17' },
  exported_at: '2024-01-01T00:00:00+00:00',
  features: ['eula'],
  file_denylist: ['*.log'],
  meta: { update_url: 'https://example.com/egg.json', version },
  name: 'Legacy',
  scripts: {
    installation: { container: 'alpine', entrypoint: 'ash', script: 'echo' }
  },
  startup: 'java -jar server.jar',
  variables: [
    {
      default_value: 'a',
      description: 'first',
      env_variable: 'FIRST',
      field_type: 'text',
      name: 'First',
      rules: 'required|string|max:20',
      user_editable: true,
      user_viewable: true
    },
    {
      default_value: 'b',
      description: 'second',
      env_variable: 'SECOND',
      name: 'Second',
      rules: ['nullable'],
      user_editable: false,
      user_viewable: false
    }
  ]
});

// Pelican-eggs/games-standalone SuperTuxKart (PLCN_v3), install script and variables trimmed.
const STK = {
  _comment: PELICAN_COMMENT,
  author: 'mattamn107@github.com',
  config: {
    files: '{}',
    logs: '{}',
    startup: '{\n    "done": "Available command:"\n}',
    stop: 'quit'
  },
  description: 'Egg for hosting a SuperTuxKart Server.',
  docker_images: {
    'ghcr.io/parkervcp/yolks:debian': 'ghcr.io/parkervcp/yolks:debian'
  },
  exported_at: '2026-01-25T15:15:37+00:00',
  features: [],
  file_denylist: [],
  image: null,
  meta: { update_url: null, version: 'PLCN_v3' },
  name: 'SuperTuxKart',
  scripts: {
    installation: {
      container: 'ghcr.io/parkervcp/installers:debian',
      entrypoint: 'bash',
      script: '#!/bin/bash\r\necho install'
    }
  },
  startup_commands: {
    Default:
      './bin/supertuxkart --port="{{SERVER_PORT}}" --difficulty={{DIFFICULTY}}'
  },
  tags: [],
  uuid: '9eec81aa-b165-4233-aa30-1244c6c96e9f',
  variables: [
    {
      default_value: 'config.xml',
      description: 'Name of the Config File.',
      env_variable: 'CONFIG_NAME',
      name: 'Config File Name',
      rules: ['required', String.raw`regex:/^([\w\d._-]+)(\.xml)$/`],
      sort: 1,
      user_editable: true,
      user_viewable: true
    },
    {
      default_value: '0',
      description:
        'Speed of the carts and AI difficulty\r\n0 = Beginner\r\n1 = Intermediate',
      env_variable: 'DIFFICULTY',
      name: 'Difficulty',
      rules: ['required', 'string', 'in:0,1,2,3'],
      sort: 2,
      user_editable: true,
      user_viewable: true
    }
  ]
};

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('option lists', () => {
  it('mirror the parser and find-mode enums', () => {
    expect(PARSER_OPTIONS.map((o) => o.value)).toEqual([...PARSERS]);
    expect(PARSER_OPTIONS.every((o) => o.label === o.value)).toBe(true);
    expect(FIND_MODE_OPTIONS.map((o) => o.value)).toEqual([
      'string',
      'number',
      'boolean',
      'map'
    ]);
  });

  it('KNOWN_FEATURES covers exactly the Pelican feature flags', () => {
    expect(KNOWN_FEATURES.map((f) => f.key)).toEqual([...FEATURES]);
    expect(new Set(KNOWN_FEATURES.map((f) => f.formKey)).size).toBe(
      FEATURES.length
    );
  });
});

describe('STAGE_KEYS', () => {
  it('partitions every eggSettings key across the five stages', () => {
    const all = STAGE_KEYS.flat();
    expect(STAGE_KEYS).toHaveLength(5);
    expect(new Set(all).size).toBe(all.length);
    const keys = Object.keys(base());
    expect(new Set(all)).toEqual(new Set(keys));
  });

  it('assigns the stage-specific keys', () => {
    expect(STAGE_KEYS[0]).toContain('name');
    expect(STAGE_KEYS[1]).toEqual(['variables']);
    expect(STAGE_KEYS[2]).toEqual([
      'scriptsContainer',
      'scriptsEntrypoint',
      'scriptsScript'
    ]);
    expect(STAGE_KEYS[3]).toContain('dockerImages');
    expect(STAGE_KEYS[4]).toContain('configFiles');
  });
});

describe('stageForm', () => {
  it('returns the same SuperForm instance', () => {
    const form = { form: {} } as never;
    expect(stageForm(form)).toBe(form);
  });
});

describe('slugify', () => {
  it.each([
    ['Paper', 'paper'],
    ['Minecraft: Java Edition', 'minecraft-java-edition'],
    ['  --Hello__World!!  ', 'hello-world'],
    ['ÄÖÜ', 'egg'],
    ['', 'egg'],
    ['a1 b2', 'a1-b2']
  ])('%j -> %j', (input, out) => {
    expect(slugify(input)).toBe(out);
  });
});

describe('apiErrorMessage', () => {
  it.each([
    ['Duplicate environment variable', 'Duplicate environment variable'],
    ['', 'fallback'],
    ['   ', 'fallback'],
    [{}, 'fallback'],
    [undefined, 'fallback'],
    [null, 'fallback'],
    [42, 'fallback']
  ])('%j -> %j', (error, out) => {
    expect(apiErrorMessage(error, 'fallback')).toBe(out);
  });
});

describe('source version', () => {
  it.each(['PTDL_v1', 'PTDL_v2', 'PLCN_v1', 'PLCN_v2', 'PLCN_v3'])(
    'eggVersion reports %s from meta.version',
    (version) => {
      expect(eggVersion({ meta: { version } })).toBe(version);
    }
  );

  it('eggVersion treats an API EggInfo as PLCN_v3', () => {
    expect(eggVersion(clone(eggInfo))).toBe('PLCN_v3');
  });

  it.each([
    ['unknown version', { meta: { version: 'PTDL_v9' } }],
    ['null', null],
    ['string', 'egg']
  ])('eggVersion rejects %s', (_, raw) => {
    expect(() => eggVersion(raw)).toThrow('The file format is not recognized.');
  });

  it.each(['PTDL_v2', 'PLCN_v1', 'PLCN_v2'])(
    'importEgg returns the formatted form and source version %s',
    (version) => {
      const raw = legacyV2(version);
      expect(importEgg(raw)).toEqual({
        form: formatData(legacyV2(version)),
        sourceVersion: version
      });
    }
  );

  it('importEgg reports PLCN_v3 for a current file', () => {
    const raw = {
      ...clone(eggInfo),
      meta: { update_url: null, version: 'PLCN_v3' }
    };
    expect(importEgg(raw).sourceVersion).toBe('PLCN_v3');
  });

  it('readEggFile parses a JSON file', async () => {
    const file = new File([JSON.stringify(legacyV2('PTDL_v2'))], 'egg.json');
    const out = await readEggFile(file);
    expect(out.sourceVersion).toBe('PTDL_v2');
    expect(out.form.name).toBe('Legacy');
  });

  it.each([
    ['invalid JSON', '{nope', 'Invalid egg file'],
    [
      'unknown version',
      '{"meta":{"version":"X"}}',
      'The file format is not recognized.'
    ]
  ])('readEggFile rejects %s', async (_, text, message) => {
    await expect(readEggFile(new File([text], 'egg.json'))).rejects.toThrow(
      message
    );
  });
});

describe('upgradeEgg', () => {
  it('passes PLCN_v3 through unchanged', () => {
    const egg = {
      ...clone(eggInfo),
      meta: { update_url: null, version: 'PLCN_v3' }
    };
    expect(upgradeEgg(egg)).toBe(egg);
  });

  it('treats an API EggInfo (no meta.version) as PLCN_v3', () => {
    const egg = clone(eggInfo);
    expect(upgradeEgg(egg)).toBe(egg);
    const noMeta = { ...clone(eggInfo), meta: undefined };
    expect(upgradeEgg(noMeta)).toBe(noMeta);
  });

  it.each([
    ['unknown version', { meta: { version: 'PTDL_v9' }, startup_commands: {} }],
    ['missing meta', { name: 'x' }],
    ['startup_commands not an object', { startup_commands: 'x' }],
    ['null', null],
    ['undefined', undefined],
    ['string', 'egg'],
    ['array', []]
  ])('rejects %s', (_, raw) => {
    expect(() => upgradeEgg(raw)).toThrow('The file format is not recognized.');
  });

  it.each(['PTDL_v2', 'PLCN_v1', 'PLCN_v2'])('upgrades %s', (version) => {
    const out = upgradeEgg(legacyV2(version));
    expect(out).toHaveProperty('_comment', PELICAN_COMMENT);
    expect(out.meta).toEqual({
      update_url: 'https://example.com/egg.json',
      version: 'PLCN_v3'
    });
    expect(out.uuid).toBe('');
    expect(out.icon).toBeNull();
    expect(out.tags).toEqual([]);
    expect(out.features).toEqual(['eula']);
    expect(out.exported_at).toBe('2024-01-01T00:00:00+00:00');
    expect(out.docker_images).toEqual({ 'Java 17': 'ghcr.io/yolks:java_17' });
    expect(out.file_denylist).toEqual(['*.log']);
    expect(out.startup_commands).toEqual({ Default: 'java -jar server.jar' });
    expect(out.variables).toEqual([
      {
        default_value: 'a',
        description: 'first',
        env_variable: 'FIRST',
        name: 'First',
        rules: ['required', 'string', 'max:20'],
        sort: 1,
        user_editable: true,
        user_viewable: true
      },
      {
        default_value: 'b',
        description: 'second',
        env_variable: 'SECOND',
        name: 'Second',
        rules: ['nullable'],
        sort: 2,
        user_editable: false,
        user_viewable: false
      }
    ]);
    expect(JSON.parse(out.config.files as string)).toEqual({
      'server.properties': {
        find: {
          'server-ip': '{{server.allocations.default.ip}}',
          'server-port': '{{server.allocations.default.port}}'
        },
        parser: 'properties'
      }
    });
    expect(out.config.stop).toBe('stop');
  });

  it('defaults missing legacy fields', () => {
    const raw = legacyV2('PTDL_v2') as Record<string, unknown>;
    delete raw.features;
    delete raw.file_denylist;
    delete raw.variables;
    raw.description = undefined;
    raw.meta = { version: 'PTDL_v2' };
    raw.config = { files: null, logs: null, startup: null, stop: null };
    const out = upgradeEgg(raw);
    expect(out.features).toBeNull();
    expect(out.file_denylist).toEqual([]);
    expect(out.variables).toEqual([]);
    expect(out.description).toBeNull();
    expect(out.meta.update_url).toBeNull();
    expect(out.config.files).toBeNull();
  });

  it('treats missing variable rules as empty', () => {
    const raw = legacyV2('PTDL_v2');
    (raw.variables[0] as Record<string, unknown>).rules = undefined;
    expect(upgradeEgg(raw).variables[0].rules).toEqual([]);
  });

  it('upgrades object-form config.files too', () => {
    const raw = legacyV2('PTDL_v2');
    (raw.config as Record<string, unknown>).files = {
      'a.yml': {
        find: { ip: '{{server.build.env.SERVER_IP}}' },
        parser: 'yaml'
      }
    };
    expect(JSON.parse(upgradeEgg(raw).config.files as string)).toEqual({
      'a.yml': {
        find: { ip: '{{server.allocations.default.ip}}' },
        parser: 'yaml'
      }
    });
  });

  it.each([
    ['server.build.env.SERVER_IP', 'server.allocations.default.ip'],
    ['server.build.default.ip', 'server.allocations.default.ip'],
    ['server.build.env.SERVER_PORT', 'server.allocations.default.port'],
    ['server.build.default.port', 'server.allocations.default.port'],
    ['server.build.env.SERVER_MEMORY', 'server.build.memory_limit'],
    ['server.build.memory', 'server.build.memory_limit'],
    ['server.build.memory_limit', 'server.build.memory_limit'],
    ['server.build.env.FOO', 'server.environment.FOO'],
    ['server.build.environment.FOO', 'server.environment.FOO']
  ])('rewrites placeholder %s -> %s', (from, to) => {
    const raw = legacyV2('PTDL_v2');
    raw.config.files = `{"f":{"parser":"file","find":{"k":"{{${from}}} {{${from}}}"}}}`;
    expect(JSON.parse(upgradeEgg(raw).config.files as string).f.find.k).toBe(
      `{{${to}}} {{${to}}}`
    );
  });

  it.each([
    [
      'images[]',
      { images: ['img:a', 'img:b'] },
      { 'img:a': 'img:a', 'img:b': 'img:b' }
    ],
    ['image', { image: 'img:solo' }, { 'img:solo': 'img:solo' }],
    [
      'empty images[] falls back to image',
      { image: 'img:x', images: [] },
      { 'img:x': 'img:x' }
    ],
    ['neither', {}, { nil: 'nil' }]
  ])('upgrades PTDL_v1 %s', (_, images, expected) => {
    const raw = legacyV2('PTDL_v1') as Record<string, unknown>;
    delete raw.docker_images;
    Object.assign(raw, images);
    expect(upgradeEgg(raw).docker_images).toEqual(expected);
  });
});

describe('formatData', () => {
  it('maps the API EggInfo shape (config objects) to the form', () => {
    expect(base()).toEqual({
      author: 'dev@example.com',
      configFiles: [
        {
          find: [
            {
              key: 'server-port',
              map: [],
              mode: ['string'],
              value: '{{server.allocations.default.port}}'
            }
          ],
          parser: ['properties'],
          path: 'server.properties'
        }
      ],
      configLogsCustom: false,
      configLogsLocation: '',
      configStartupDone: [')! For help, type '],
      configStop: 'stop',
      configStripAnsi: false,
      configUserInteraction: [],
      customFeatures: [],
      description: 'Paper Minecraft server',
      dockerImages: [
        { image: 'ghcr.io/pelican-eggs/yolks:java_21', label: 'Java 21' }
      ],
      featureEula: true,
      featureGslToken: false,
      featureJavaVersion: false,
      featurePidLimit: false,
      featureSteamDiskSpace: false,
      fileDenylist: [],
      icon: '',
      name: 'Paper',
      scriptsContainer: 'ghcr.io/pelican-eggs/installers:java_21',
      scriptsEntrypoint: 'bash',
      scriptsScript: 'echo install',
      startupCommands: [
        { command: 'java -jar {{SERVER_JARFILE}}', label: 'Default' }
      ],
      tags: ['minecraft'],
      updateUrl: '',
      variables: [
        {
          defaultValue: 'server.jar',
          description: 'Jar to run',
          envVariable: 'SERVER_JARFILE',
          name: 'Server Jar File',
          rules: ['required', 'string'],
          userEditable: true,
          userViewable: true
        }
      ]
    });
  });

  it('parses JSON-string config the same as objects', () => {
    const egg = clone(eggInfo) as Record<string, unknown>;
    egg.config = {
      files: JSON.stringify(eggInfo.config.files),
      logs: JSON.stringify(eggInfo.config.logs),
      startup: JSON.stringify(eggInfo.config.startup),
      stop: 'stop'
    };
    expect(formatData(egg)).toEqual(base());
  });

  it('falls back to defaults on invalid or missing config', () => {
    const egg = clone(eggInfo) as Record<string, unknown>;
    egg.config = {
      files: '{not json',
      logs: null,
      startup: 'nope',
      stop: null
    };
    const out = formatData(egg);
    expect(out.configFiles).toEqual([]);
    expect(out.configStartupDone).toEqual([]);
    expect(out.configLogsCustom).toBe(false);
    expect(out.configLogsLocation).toBe('');
    expect(out.configStop).toBe('');
    delete egg.config;
    expect(formatData(egg).configFiles).toEqual([]);
  });

  it.each([
    ['string', 'Done', ['Done']],
    ['array', ['a', 'regex:^b$'], ['a', 'regex:^b$']],
    ['empty string', '', []],
    ['missing', undefined, []]
  ])('reads startup.done as %s', (_, done, expected) => {
    const egg = clone(eggInfo);
    (egg.config as Record<string, unknown>).startup = { done };
    expect(formatData(egg).configStartupDone).toEqual(expected);
  });

  it('reads strip_ansi, userInteraction and logs', () => {
    const egg = clone(eggInfo);
    (egg.config as Record<string, unknown>).startup = {
      done: 'x',
      strip_ansi: true,
      userInteraction: ['Agree?']
    };
    (egg.config as Record<string, unknown>).logs = {
      custom: true,
      location: 'logs/latest.log'
    };
    const out = formatData(egg);
    expect(out.configStripAnsi).toBe(true);
    expect(out.configUserInteraction).toEqual(['Agree?']);
    expect(out.configLogsCustom).toBe(true);
    expect(out.configLogsLocation).toBe('logs/latest.log');
  });

  it('maps find value types to modes', () => {
    const egg = clone(eggInfo);
    (egg.config as Record<string, unknown>).files = {
      f: {
        find: {
          bool: false,
          map: { '127.0.0.1': '{{ip}}', num: 3, other: null },
          nil: null,
          num: 25_565,
          str: ' keep '
        }
      }
    };
    const [file] = formatData(egg).configFiles;
    expect(file.parser).toEqual(['file']);
    expect(file.find).toEqual([
      { key: 'bool', map: [], mode: ['boolean'], value: 'false' },
      {
        key: 'map',
        map: [
          { match: '127.0.0.1', replace: '{{ip}}' },
          { match: 'num', replace: '3' },
          { match: 'other', replace: '' }
        ],
        mode: ['map'],
        value: ''
      },
      { key: 'nil', map: [], mode: ['string'], value: '' },
      { key: 'num', map: [], mode: ['number'], value: '25565' },
      { key: 'str', map: [], mode: ['string'], value: ' keep ' }
    ]);
  });

  it('splits known and custom features', () => {
    const egg = {
      ...clone(eggInfo),
      features: ['pid_limit', 'gsl_token', 'my_flag']
    };
    const out = formatData(egg);
    expect(out.featurePidLimit).toBe(true);
    expect(out.featureGslToken).toBe(true);
    expect(out.featureEula).toBe(false);
    expect(out.customFeatures).toEqual(['my_flag']);
    expect(
      formatData({ ...clone(eggInfo), features: null }).customFeatures
    ).toEqual([]);
  });

  it('orders variables by sort, unsorted last, ties stable', () => {
    const v = (env: string, sort: number | null | undefined) => ({
      ...eggInfo.variables[0],
      env_variable: env,
      sort
    });
    const egg = {
      ...clone(eggInfo),
      variables: [
        v('NONE_A', null),
        v('THREE', 3),
        v('ONE_A', 1),
        v('NONE_B', undefined),
        v('ONE_B', 1)
      ]
    };
    expect(formatData(egg).variables.map((x) => x.envVariable)).toEqual([
      'ONE_A',
      'ONE_B',
      'THREE',
      'NONE_A',
      'NONE_B'
    ]);
  });

  it('defaults missing variable fields', () => {
    const egg = { ...clone(eggInfo), variables: [{}] };
    expect(formatData(egg).variables).toEqual([
      {
        defaultValue: '',
        description: '',
        envVariable: '',
        name: '',
        rules: [],
        userEditable: false,
        userViewable: false
      }
    ]);
  });

  it.each([
    [
      'icon',
      { icon: 'data:image/png;base64,AAA' },
      'data:image/png;base64,AAA'
    ],
    [
      'legacy data: image',
      { icon: null, image: 'data:image/webp;base64,BBB' },
      'data:image/webp;base64,BBB'
    ],
    ['non-data image', { icon: null, image: 'https://x/y.png' }, ''],
    ['none', { icon: null, image: null }, '']
  ])('reads the icon from %s', (_, over, expected) => {
    expect(formatData({ ...clone(eggInfo), ...over }).icon).toBe(expected);
  });

  it('adds a blank row when images / startup commands are empty', () => {
    const out = formatData({
      ...clone(eggInfo),
      docker_images: {},
      startup_commands: {}
    });
    expect(out.dockerImages).toEqual([{ image: '', label: '' }]);
    expect(out.startupCommands).toEqual([{ command: '', label: 'Default' }]);
  });

  it('keeps docker image and startup command order', () => {
    const out = formatData({
      ...clone(eggInfo),
      // oxlint-disable-next-line eslint/sort-keys -- unsorted on purpose: checks key order is kept
      docker_images: { Z: 'z', A: 'a', M: 'm' },
      // oxlint-disable-next-line eslint/sort-keys -- unsorted on purpose: checks key order is kept
      startup_commands: { Second: '2', First: '1' }
    });
    expect(out.dockerImages.map((r) => r.label)).toEqual(['Z', 'A', 'M']);
    expect(out.startupCommands.map((r) => r.label)).toEqual([
      'Second',
      'First'
    ]);
  });

  it('defaults missing scalar fields', () => {
    const out = formatData({ startup_commands: {} });
    expect(out.name).toBe('');
    expect(out.author).toBe('');
    expect(out.description).toBe('');
    expect(out.tags).toEqual([]);
    expect(out.fileDenylist).toEqual([]);
    expect(out.scriptsContainer).toBe('');
    expect(out.scriptsEntrypoint).toBe('bash');
    expect(out.scriptsScript).toBe('');
    expect(out.updateUrl).toBe('');
    expect(out.variables).toEqual([]);
  });

  it('reads meta.update_url', () => {
    const out = formatData({
      ...clone(eggInfo),
      meta: { update_url: 'https://u' }
    });
    expect(out.updateUrl).toBe('https://u');
  });

  it('throws for unknown files', () => {
    expect(() => formatData({ meta: { version: 'x' } })).toThrow();
  });
});

describe('reformatData', () => {
  it('exports PLCN_v3 in panel key order', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-01-02T03:04:05.000Z'));
    const out = reformatData(base(), 'PLCN_v3', 'egg-uuid');
    expect(Object.keys(out)).toEqual([
      '_comment',
      'meta',
      'exported_at',
      'name',
      'author',
      'uuid',
      'description',
      'icon',
      'tags',
      'features',
      'docker_images',
      'file_denylist',
      'startup_commands',
      'config',
      'scripts',
      'variables'
    ]);
    expect(out).toMatchObject({
      _comment: PELICAN_COMMENT,
      author: 'dev@example.com',
      description: 'Paper Minecraft server',
      docker_images: { 'Java 21': 'ghcr.io/pelican-eggs/yolks:java_21' },
      exported_at: '2026-01-02T03:04:05.000Z',
      features: ['eula'],
      icon: null,
      meta: { update_url: null, version: 'PLCN_v3' },
      name: 'Paper',
      scripts: {
        installation: {
          container: 'ghcr.io/pelican-eggs/installers:java_21',
          entrypoint: 'bash',
          script: 'echo install'
        }
      },
      startup_commands: { Default: 'java -jar {{SERVER_JARFILE}}' },
      tags: ['minecraft'],
      uuid: 'egg-uuid',
      variables: [
        {
          default_value: 'server.jar',
          description: 'Jar to run',
          env_variable: 'SERVER_JARFILE',
          name: 'Server Jar File',
          rules: ['required', 'string'],
          sort: 1,
          user_editable: true,
          user_viewable: true
        }
      ]
    });
  });

  it('defaults to PLCN_v3 with a random uuid', () => {
    vi.spyOn(crypto, 'randomUUID').mockReturnValue('1-2-3-4-5');
    const out = reformatData(base());
    expect(out.meta.version).toBe('PLCN_v3');
    expect(out.uuid).toBe('1-2-3-4-5');
  });

  it('encodes config as 4-space JSON strings', () => {
    const { config } = reformatData(base(), 'PLCN_v3', 'u');
    expect(config.files).toBe(
      JSON.stringify(
        {
          // oxlint-disable-next-line eslint/sort-keys -- the export writes parser before find
          'server.properties': {
            parser: 'properties',
            find: { 'server-port': '{{server.allocations.default.port}}' }
          }
        },
        null,
        4
      )
    );
    expect(config.startup).toBe(
      JSON.stringify({ done: ')! For help, type ' }, null, 4)
    );
    expect(config.logs).toBe('{}');
    expect(config.stop).toBe('stop');
  });

  it('writes done as an array when there are several', () => {
    const form = { ...base(), configStartupDone: ['a', 'b'] };
    expect(
      JSON.parse(reformatData(form, 'PLCN_v3', 'u').config.startup as string)
    ).toEqual({
      done: ['a', 'b']
    });
  });

  it('adds userInteraction / strip_ansi / logs only when set', () => {
    const form = {
      ...base(),
      configLogsCustom: true,
      configLogsLocation: 'l.log',
      configStripAnsi: true,
      configUserInteraction: ['Agree?']
    };
    const { config } = reformatData(form, 'PLCN_v3', 'u');
    expect(JSON.parse(config.startup as string)).toEqual({
      done: ')! For help, type ',
      strip_ansi: true,
      userInteraction: ['Agree?']
    });
    expect(JSON.parse(config.logs as string)).toEqual({
      custom: true,
      location: 'l.log'
    });
    const locationOnly = reformatData(
      { ...base(), configLogsLocation: 'x' },
      'PLCN_v3',
      'u'
    );
    expect(JSON.parse(locationOnly.config.logs as string)).toEqual({
      custom: false,
      location: 'x'
    });
  });

  it('nulls empty optional strings and trims the update url', () => {
    const form = {
      ...base(),
      configStop: '',
      description: '',
      icon: '',
      scriptsScript: '',
      updateUrl: '   '
    };
    const out = reformatData(form, 'PLCN_v3', 'u');
    expect(out.config.stop).toBeNull();
    expect(out.description).toBeNull();
    expect(out.icon).toBeNull();
    expect(out.scripts.installation.script).toBeNull();
    expect(out.meta.update_url).toBeNull();
    expect(
      reformatData({ ...base(), updateUrl: ' https://u ' }, 'PLCN_v3', 'u').meta
        .update_url
    ).toBe('https://u');
  });

  it('orders features known-first then custom', () => {
    const form = {
      ...base(),
      customFeatures: ['custom'],
      featureEula: true,
      featureSteamDiskSpace: true
    };
    expect(reformatData(form, 'PLCN_v3', 'u').features).toEqual([
      'eula',
      'steam_disk_space',
      'custom'
    ]);
  });

  it('converts find entries back to typed values', () => {
    const form = base();
    form.configFiles = [
      {
        find: [
          { key: 's', map: [], mode: ['string'], value: '25565' },
          { key: 'n', map: [], mode: ['number'], value: '25565' },
          { key: 't', map: [], mode: ['boolean'], value: 'true' },
          { key: 'f', map: [], mode: ['boolean'], value: 'false' },
          {
            key: 'm',
            map: [{ match: 'a', replace: 'b' }],
            mode: ['map'],
            value: 'ignored'
          }
        ],
        parser: ['yaml'],
        path: 'c.yml'
      }
    ];
    expect(
      JSON.parse(reformatData(form, 'PLCN_v3', 'u').config.files as string)
    ).toEqual({
      'c.yml': {
        find: { f: false, m: { a: 'b' }, n: 25_565, s: '25565', t: true },
        parser: 'yaml'
      }
    });
  });

  it('assigns variable sort by position', () => {
    const form = {
      ...base(),
      variables: [
        variable({ envVariable: 'A' }),
        variable({ envVariable: 'B' })
      ]
    };
    expect(
      reformatData(form, 'PLCN_v3', 'u').variables.map((v) => [
        v.env_variable,
        v.sort
      ])
    ).toEqual([
      ['A', 1],
      ['B', 2]
    ]);
  });

  it('exports PTDL_v2', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-01-02T03:04:05.000Z'));
    const form = {
      ...base(),
      startupCommands: [
        { command: 'first', label: 'Default' },
        { command: 'second', label: 'Other' }
      ],
      updateUrl: 'https://u'
    };
    const out = reformatData(form, 'PTDL_v2');
    expect(Object.keys(out)).toEqual([
      '_comment',
      'meta',
      'exported_at',
      'name',
      'author',
      'description',
      'features',
      'docker_images',
      'file_denylist',
      'startup',
      'config',
      'scripts',
      'variables'
    ]);
    expect(out).toHaveProperty('_comment', PTERODACTYL_COMMENT);
    expect(out.meta).toEqual({ update_url: 'https://u', version: 'PTDL_v2' });
    expect(out.exported_at).toBe('2026-01-02T03:04:05.000Z');
    expect(out.startup).toBe('first');
    expect(out.variables).toEqual([
      {
        default_value: 'server.jar',
        description: 'Jar to run',
        env_variable: 'SERVER_JARFILE',
        field_type: 'text',
        name: 'Server Jar File',
        rules: 'required|string',
        user_editable: true,
        user_viewable: true
      }
    ]);
    expect(
      reformatData({ ...base(), startupCommands: [] }, 'PTDL_v2').startup
    ).toBe('');
  });

  it('round-trips through formatData for both formats', () => {
    const form = {
      ...base(),
      configStartupDone: ['a', 'b'],
      configStripAnsi: true,
      configUserInteraction: ['q'],
      customFeatures: ['x'],
      variables: [
        variable({ envVariable: 'A' }),
        variable({ envVariable: 'B', rules: ['a', 'b'] })
      ]
    };
    expect(formatData(reformatData(form, 'PLCN_v3', 'u'))).toEqual(form);
    // PTDL_v2 has no tags/icon and only one startup command.
    expect(formatData(reformatData(form, 'PTDL_v2'))).toEqual({
      ...form,
      icon: '',
      tags: []
    });
  });

  it('round-trips a real PLCN_v3 file, image: null becoming icon', () => {
    const out = reformatData(formatData(STK), 'PLCN_v3', STK.uuid);
    expect(out).not.toHaveProperty('image');
    expect(out).toEqual({
      ...STK,
      exported_at: out.exported_at,
      icon: null,
      image: undefined
    });
    expect(Object.keys(out.variables[0])).toEqual([
      'name',
      'description',
      'env_variable',
      'default_value',
      'user_viewable',
      'user_editable',
      'rules',
      'sort'
    ]);
  });
});

describe('toEggBody', () => {
  it('drops the file-only fields and keeps update_url', () => {
    const body = toEggBody({ ...base(), updateUrl: 'https://u' });
    expect(body).not.toHaveProperty('_comment');
    expect(body).not.toHaveProperty('exported_at');
    expect(body).not.toHaveProperty('uuid');
    expect(body).not.toHaveProperty('image');
    expect(body.meta).toEqual({ update_url: 'https://u' });
    expect(toEggBody(base()).meta).toEqual({ update_url: null });
  });

  it('matches the PLCN_v3 export otherwise', () => {
    const form = base();
    // `toEqual` treats undefined properties as missing.
    expect(toEggBody(form)).toEqual({
      ...reformatData(form, 'PLCN_v3', 'u'),
      _comment: undefined,
      exported_at: undefined,
      meta: { update_url: null },
      uuid: undefined
    });
  });

  it('sends a sort on every variable', () => {
    const body = toEggBody({
      ...base(),
      variables: [
        variable({ envVariable: 'A' }),
        variable({ envVariable: 'B' })
      ]
    });
    expect(body.variables.map((v) => v.sort)).toEqual([1, 2]);
  });

  it('round-trips with an API EggInfo', () => {
    const form = base();
    expect(formatData({ ...toEggBody(form), uuid: 'egg-paper' })).toEqual(form);
  });
});

describe('eggSettings validation', () => {
  it('accepts a valid egg', () => {
    expect(issues(eggSettings, base())).toEqual({});
  });

  it.each([
    ['name', { name: '' }, 'Egg name is required'],
    [
      'name',
      { name: 'x'.repeat(256) },
      'Egg name must be 255 characters or fewer'
    ],
    [
      'author',
      { author: 'not-an-email' },
      'Author must be a valid email address'
    ],
    [
      'scriptsContainer',
      { scriptsContainer: '' },
      'Install container image is required'
    ],
    [
      'scriptsEntrypoint',
      { scriptsEntrypoint: '' },
      'Entrypoint shell is required'
    ],
    ['configStop', { configStop: '' }, 'Stop command is required'],
    [
      'configStartupDone',
      { configStartupDone: [] },
      'At least one boot-detection string is required'
    ],
    ['configStartupDone.0', { configStartupDone: [''] }, 'Must not be empty'],
    [
      'dockerImages',
      { dockerImages: [] },
      'At least one docker image is required'
    ],
    [
      'dockerImages.0.image',
      { dockerImages: [{ image: '', label: 'L' }] },
      'Image reference is required'
    ],
    [
      'dockerImages.0.label',
      { dockerImages: [{ image: 'i', label: '' }] },
      'Label is required'
    ],
    [
      'startupCommands',
      { startupCommands: [] },
      'At least one startup command is required'
    ],
    [
      'startupCommands.0.command',
      { startupCommands: [{ command: '', label: 'L' }] },
      'Command is required'
    ],
    [
      'startupCommands.0.label',
      { startupCommands: [{ command: 'c', label: '' }] },
      'Label is required'
    ]
  ])('rejects %s for %j', (path, over, message) => {
    expect(issues(eggSettings, { ...base(), ...over })[path]).toContain(
      message
    );
  });

  it('accepts boundary values', () => {
    expect(issues(eggSettings, { ...base(), name: 'x'.repeat(255) })).toEqual(
      {}
    );
    expect(
      issues(eggSettings, { ...base(), configStartupDone: [' '] })
    ).toEqual({});
    expect(issues(eggSettings, { ...base(), name: 'a' })).toEqual({});
  });

  it('accepts an egg without variables or config files', () => {
    expect(
      issues(eggSettings, { ...base(), configFiles: [], variables: [] })
    ).toEqual({});
  });
});

describe('variable validation', () => {
  const varIssues = (...vars: ReturnType<typeof variable>[]) =>
    issues(variablesStage, { variables: vars });

  it.each(RESERVED_ENV_NAMES)('rejects reserved env %s', (env) => {
    expect(varIssues(variable({ envVariable: env }))).toEqual({
      'variables.0.envVariable': ['Reserved name — choose another.']
    });
  });

  it('matches reserved names exact-case', () => {
    expect(varIssues(variable({ envVariable: 'server_ip' }))).toEqual({});
  });

  it('flags only the later duplicate env variable', () => {
    expect(
      varIssues(
        variable({ envVariable: 'A' }),
        variable({ envVariable: 'B' }),
        variable({ envVariable: 'A' })
      )
    ).toEqual({ 'variables.2.envVariable': ['Duplicate env variable name.'] });
  });

  it('reports a reserved duplicate as reserved only', () => {
    expect(
      varIssues(
        variable({ envVariable: 'TZ' }),
        variable({ envVariable: 'TZ' })
      )
    ).toEqual({
      'variables.0.envVariable': ['Reserved name — choose another.'],
      'variables.1.envVariable': ['Reserved name — choose another.']
    });
  });

  it.each([
    ['envVariable', { envVariable: '' }, 'Env variable is required'],
    [
      'envVariable',
      { envVariable: 'HAS SPACE' },
      'Letters, numbers, dash and underscore only'
    ],
    [
      'envVariable',
      { envVariable: 'bad.dot' },
      'Letters, numbers, dash and underscore only'
    ],
    ['envVariable', { envVariable: 'X'.repeat(256) }, 'Max 255 characters'],
    ['name', { name: '' }, 'Name is required'],
    ['name', { name: 'n'.repeat(256) }, 'Max 255 characters'],
    ['rules', { rules: [] }, 'At least one validation rule is required']
  ])('rejects variable %s %j', (field, over, message) => {
    expect(varIssues(variable(over))[`variables.0.${field}`]).toContain(
      message
    );
  });

  it.each(['a', 'A-b_9', 'X'.repeat(255)])('accepts env %s', (env) => {
    expect(varIssues(variable({ envVariable: env }))).toEqual({});
  });

  it('applies the same checks in eggSettings', () => {
    expect(
      issues(eggSettings, {
        ...base(),
        variables: [
          variable({ envVariable: 'A' }),
          variable({ envVariable: 'A' })
        ]
      })
    ).toEqual({ 'variables.1.envVariable': ['Duplicate env variable name.'] });
  });
});

describe('runtime validation', () => {
  const runtime = (over: object = {}) => ({
    configStartupDone: ['Done'],
    configStop: 'stop',
    dockerImages: [{ image: 'i', label: 'L' }],
    startupCommands: [{ command: 'a', label: 'Default' }],
    ...over
  });

  it('accepts distinct startup commands', () => {
    expect(
      issues(
        runtimeStage,
        runtime({
          startupCommands: [
            { command: 'a', label: 'A' },
            { command: 'b', label: 'B' }
          ]
        })
      )
    ).toEqual({});
  });

  it('rejects duplicate labels and commands on the later row', () => {
    expect(
      issues(
        runtimeStage,
        runtime({
          startupCommands: [
            { command: 'a', label: 'A' },
            { command: 'a', label: 'A' }
          ]
        })
      )
    ).toEqual({
      'startupCommands.1.command': ['Startup commands must be distinct.'],
      'startupCommands.1.label': ['Duplicate label.']
    });
  });

  it('rejects a duplicate command with a distinct label', () => {
    expect(
      issues(
        runtimeStage,
        runtime({
          startupCommands: [
            { command: 'a', label: 'A' },
            { command: 'a', label: 'B' }
          ]
        })
      )
    ).toEqual({
      'startupCommands.1.command': ['Startup commands must be distinct.']
    });
  });

  it('rejects the blank rows the form starts with', () => {
    // Zod 4 does not validate `.default()` values, so pass the form's initial rows explicitly.
    const { dockerImages, startupCommands } = runtimeStage.parse({});
    expect(dockerImages).toEqual([{ image: '', label: '' }]);
    expect(startupCommands).toEqual([{ command: '', label: 'Default' }]);
    expect(
      issues(runtimeStage, {
        configStartupDone: [],
        configStop: '',
        dockerImages,
        startupCommands
      })
    ).toEqual({
      configStartupDone: ['At least one boot-detection string is required'],
      configStop: ['Stop command is required'],
      'dockerImages.0.image': ['Image reference is required'],
      'dockerImages.0.label': ['Label is required'],
      'startupCommands.0.command': ['Command is required']
    });
  });
});

describe('config validation', () => {
  const file = (find: object[] = [], path = 'f') => ({
    find,
    parser: ['file'],
    path
  });
  const entry = (mode: string, value: string) => ({
    key: 'k',
    map: [],
    mode: [mode],
    value
  });
  const cfg = (...files: object[]) => ({ configFiles: files });
  const cfgIssues = (...files: object[]) => issues(configStage, cfg(...files));
  const entryIssues = (...entries: object[]) => cfgIssues(file(entries));

  it('rejects duplicate file paths on the later file', () => {
    expect(cfgIssues(file([], 'a'), file([], 'b'), file([], 'a'))).toEqual({
      'configFiles.2.path': ['Duplicate file path.']
    });
  });

  it('requires a path and a key', () => {
    expect(
      cfgIssues({
        find: [{ ...entry('string', ''), key: '' }],
        parser: ['file'],
        path: ''
      })
    ).toEqual({
      'configFiles.0.find.0.key': ['Key is required'],
      'configFiles.0.path': ['File path is required']
    });
  });

  it.each(['25565', '-1', '1.5', '1e3', ' 7 '])(
    'accepts number %j',
    (value) => {
      expect(entryIssues(entry('number', value))).toEqual({});
    }
  );

  it.each(['', ' ', 'abc', 'Infinity', 'NaN', '1px'])(
    'rejects number %j',
    (value) => {
      expect(entryIssues(entry('number', value))).toEqual({
        'configFiles.0.find.0.value': ['Must be a number.']
      });
    }
  );

  it.each(['true', 'false'])('accepts boolean %s', (value) => {
    expect(entryIssues(entry('boolean', value))).toEqual({});
  });

  it.each(['', 'yes', 'True', '1'])('rejects boolean %j', (value) => {
    expect(entryIssues(entry('boolean', value))).toEqual({
      'configFiles.0.find.0.value': ['Must be true or false.']
    });
  });

  it.each([
    ['string', ''],
    ['map', 'anything']
  ])('does not check %s values', (mode, value) => {
    expect(entryIssues(entry(mode, value))).toEqual({});
  });

  it.each([
    ['no mode', []],
    ['two modes', ['string', 'number']],
    ['unknown mode', ['date']]
  ])('rejects %s', (_, mode) => {
    const res = configStage.safeParse(
      cfg(file([{ key: 'k', map: [], mode, value: '' }]))
    );
    expect(res.success).toBe(false);
  });

  it.each([[[]], [['file', 'ini']], [['toml']]])(
    'rejects parser %j',
    (parser) => {
      expect(
        configStage.safeParse(cfg({ find: [], parser, path: 'p' })).success
      ).toBe(false);
    }
  );

  it('checks every file and entry', () => {
    const files = cfg(
      file([entry('number', 'x')], 'a'),
      file([entry('string', ''), entry('boolean', 'x')], 'b')
    );
    expect(issues(configStage, files)).toEqual({
      'configFiles.0.find.0.value': ['Must be a number.'],
      'configFiles.1.find.1.value': ['Must be true or false.']
    });
  });
});

describe('general / install stage schemas', () => {
  it('apply defaults', () => {
    expect(installStage.parse({ scriptsContainer: 'c' })).toEqual({
      scriptsContainer: 'c',
      scriptsEntrypoint: 'bash',
      scriptsScript: ''
    });
    expect(generalStage.parse({ author: 'a@b.co', name: 'n' })).toMatchObject({
      customFeatures: [],
      description: '',
      featureEula: false,
      icon: '',
      tags: [],
      updateUrl: ''
    });
  });

  it('require name and author', () => {
    expect(
      Object.keys(issues(generalStage, { author: '', name: '' })).toSorted()
    ).toEqual(['author', 'name']);
  });
});
