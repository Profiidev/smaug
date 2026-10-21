import { describe, expect, it } from 'vitest';
import type { z } from 'zod';
import {
  type GrantRow,
  LIMIT_DEFAULTS,
  SECTIONS,
  SECTION_DESCRIPTIONS,
  SECTION_KEYS,
  SECTION_TITLES,
  type ServerForm,
  defaultCommand,
  eggStartup,
  eggVariables,
  findEgg,
  fromGrantsInfo,
  fromServerInfo,
  generalStage,
  isFieldErrors,
  mapFieldErrors,
  optionsFromInfo,
  permissionsStage,
  resourcesStage,
  serverSettings,
  stageForm,
  startupStage,
  toGrantsBody,
  toServerBody,
  toUpdateBody,
  variablesEdited,
  variablesStage
} from '$routes/servers/schema.svelte';
import { options, serverInfo } from './fixtures';

const clone = <T>(v: T): T => structuredClone(v);
const base = (): ServerForm => fromServerInfo(clone(serverInfo));

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

const [paper] = options.eggs;

describe('sections', () => {
  it('lists the sections in wizard order with titles and descriptions', () => {
    expect(SECTIONS).toEqual([
      'general',
      'startup',
      'variables',
      'resources',
      'permissions'
    ]);
    for (const s of SECTIONS) {
      expect(SECTION_TITLES[s]).toBeTruthy();
      expect(SECTION_DESCRIPTIONS[s]).toBeTruthy();
    }
  });

  it('partitions every form key into exactly one section', () => {
    const keys = SECTIONS.flatMap((s) => SECTION_KEYS[s]).toSorted();
    expect(keys).toEqual(Object.keys(base()).toSorted());
    expect(new Set(keys).size).toBe(keys.length);
    expect(SECTION_KEYS.general).toEqual(
      expect.arrayContaining(['name', 'owner', 'node', 'egg', 'allocations'])
    );
  });

  it('stageForm returns the same object', () => {
    const form = {} as never;
    expect(stageForm(form)).toBe(form);
  });
});

describe('defaults', () => {
  it('parses an empty object into the create defaults', () => {
    const g = generalStage.parse({
      allocations: [{ ip: '1.2.3.4', port: 25_565 }],
      egg: ['e'],
      name: 'x',
      node: ['n'],
      owner: ['o']
    });
    expect(g.allocations).toHaveLength(1);
    expect(startupStage.parse({ image: 'img', startup: 'run' })).toEqual({
      image: 'img',
      restartOnCrash: true,
      runInstall: true,
      startOnCompletion: true,
      startup: 'run',
      startupOverridden: false,
      stopTimeout: 60
    });
    expect(resourcesStage.parse({})).toEqual({
      backupLimit: 0,
      cpu: LIMIT_DEFAULTS.cpu,
      cpuUnlimited: true,
      disk: LIMIT_DEFAULTS.disk,
      diskUnlimited: true,
      memory: LIMIT_DEFAULTS.memory,
      memoryUnlimited: true
    });
    expect(variablesStage.parse({})).toEqual({ variables: [] });
    expect(permissionsStage.parse({})).toEqual({
      groupGrants: [],
      userGrants: []
    });
  });

  // Zod returns defaults unvalidated; superforms always submits every key.
  it('defaults to one empty allocation row on port 25565', () => {
    expect(generalStage.parse({})).toEqual({
      allocations: [{ ip: '', port: 25_565 }],
      egg: [],
      name: '',
      node: [],
      owner: []
    });
  });
});

describe('generalStage', () => {
  const general = (over: Record<string, unknown> = {}) => ({
    allocations: [{ ip: '10.0.0.5', port: 25_565 }],
    egg: ['egg-paper'],
    name: 'survival',
    node: ['node-1'],
    owner: ['user-admin'],
    ...over
  });

  it('accepts a valid section', () => {
    expect(issues(generalStage, general())).toEqual({});
  });

  it.each([
    ['', 'Name is required.'],
    ['   ', 'Name is required.'],
    ['x'.repeat(192), 'Name must be 191 characters or fewer.']
  ])('rejects name %j', (name, message) => {
    expect(issues(generalStage, general({ name })).name).toEqual([message]);
  });

  it('accepts a 191 character name', () => {
    const value = general({ name: 'x'.repeat(191) });
    expect(issues(generalStage, value)).toEqual({});
  });

  it.each([
    ['owner', 'Owner is required.'],
    ['node', 'Node is required.'],
    ['egg', 'Egg is required.']
  ])('requires %s', (key, message) => {
    expect(issues(generalStage, general({ [key]: [] }))[key]).toEqual([
      message
    ]);
  });

  it('rejects an empty allocation list', () => {
    expect(
      issues(generalStage, general({ allocations: [] })).allocations
    ).toEqual(['Add at least one allocation.']);
  });

  it.each(['10.0.0.5', '0.0.0.0', '::1', '2001:db8::1', ' 10.0.0.5 '])(
    'accepts ip %j',
    (ip) => {
      expect(
        issues(generalStage, general({ allocations: [{ ip, port: 2000 }] }))
      ).toEqual({});
    }
  );

  it.each(['', 'localhost', '256.0.0.1', '10.0.0', 'gg::1'])(
    'rejects ip %j',
    (ip) => {
      expect(
        issues(generalStage, general({ allocations: [{ ip, port: 2000 }] }))[
          'allocations.0.ip'
        ]
      ).toEqual(['Enter a valid IPv4 or IPv6 address.']);
    }
  );

  it.each([1024, 65_535])('accepts port %i', (port) => {
    expect(
      issues(generalStage, general({ allocations: [{ ip: '::1', port }] }))
    ).toEqual({});
  });

  it.each([
    [1023, 'Port must be between 1024 and 65535.'],
    [65_536, 'Port must be between 1024 and 65535.'],
    [2000.5, 'Port must be a whole number.'],
    [null, 'Port is required.']
  ])('rejects port %j', (port, message) => {
    expect(
      issues(generalStage, general({ allocations: [{ ip: '::1', port }] }))[
        'allocations.0.port'
      ]
    ).toEqual([message]);
  });

  it('flags every repeat of an ip/port pair (case and space insensitive)', () => {
    const res = issues(
      generalStage,
      general({
        allocations: [
          { ip: '2001:DB8::1', port: 2000 },
          { ip: '2001:db8::1 ', port: 2000 },
          { ip: '2001:db8::1', port: 2001 },
          { ip: '2001:db8::1', port: 2000 }
        ]
      })
    );
    expect(res).toEqual({
      'allocations.1.port': ['Duplicate allocation.'],
      'allocations.3.port': ['Duplicate allocation.']
    });
  });

  const OVERLAP = 'Overlaps another allocation on the same port.';
  const pair = (a: string, b: string, portB = 2000) =>
    issues(
      generalStage,
      general({
        allocations: [
          { ip: a, port: 2000 },
          { ip: b, port: portB }
        ]
      })
    );

  it.each([
    ['0.0.0.0', '127.0.0.1'],
    ['127.0.0.1', '0.0.0.0'],
    ['::', '10.0.0.5'],
    ['10.0.0.5', '::'],
    ['::', '::1'],
    ['::1', '::'],
    ['0.0.0.0', '::'],
    ['::', '0.0.0.0'],
    ['0:0::0', '127.0.0.1']
  ])('flags %s and %s on the same port as overlapping', (a, b) => {
    expect(pair(a, b)).toEqual({ 'allocations.1.port': [OVERLAP] });
  });

  it.each([
    ['0.0.0.0', '::1'],
    ['::1', '0.0.0.0'],
    ['0.0.0.0', '::ffff:127.0.0.1'],
    ['10.0.0.5', '10.0.0.6']
  ])('accepts %s and %s on the same port', (a, b) => {
    expect(pair(a, b)).toEqual({});
  });

  it.each([
    ['0.0.0.0', '127.0.0.1'],
    ['::', '::1'],
    ['0.0.0.0', '0.0.0.0']
  ])('accepts %s and %s on different ports', (a, b) => {
    expect(pair(a, b, 2001)).toEqual({});
  });

  it.each([
    ['0.0.0.0', '0.0.0.0'],
    ['::', '0:0::0']
  ])('keeps the duplicate message for %s and %s', (a, b) => {
    expect(pair(a, b)).toEqual({
      'allocations.1.port': ['Duplicate allocation.']
    });
  });
});

describe('startupStage', () => {
  it.each([
    ['startup', 'Startup command is required.'],
    ['image', 'Image is required.']
  ])('requires %s (whitespace counts as empty)', (key, message) => {
    const value = { image: 'img', startup: 'run', [key]: '  ' };
    expect(issues(startupStage, value)[key]).toEqual([message]);
  });
});

describe('variablesStage', () => {
  const row = (over: Record<string, unknown> = {}) => ({
    defaultValue: '20',
    description: '',
    editable: true,
    env: 'MAX_PLAYERS',
    name: 'Max players',
    rules: ['required', 'integer'],
    value: '20',
    ...over
  });

  it('accepts valid values', () => {
    expect(issues(variablesStage, { variables: [row()] })).toEqual({});
  });

  it('reports the egg rule error on the value', () => {
    expect(
      issues(variablesStage, {
        variables: [row(), row({ env: 'B', value: 'abc' })]
      })
    ).toEqual({
      'variables.1.value': ['The Max players field must be an integer.']
    });
  });

  it('reports required on an empty value', () => {
    expect(issues(variablesStage, { variables: [row({ value: '' })] })).toEqual(
      { 'variables.0.value': ['The Max players field is required.'] }
    );
  });

  it('skips variables the caller cannot edit', () => {
    expect(
      issues(variablesStage, {
        variables: [row({ editable: false, value: 'abc' })]
      })
    ).toEqual({});
  });
});

describe('resourcesStage', () => {
  it('ignores hidden limits, even invalid ones', () => {
    expect(
      issues(resourcesStage, { cpu: -1, disk: null, memory: 1.5 })
    ).toEqual({});
  });

  it.each([
    ['cpu', 'cpuUnlimited', 'CPU must be 0 or more millicores.'],
    ['memory', 'memoryUnlimited', 'Memory must be 0 or more MiB.'],
    ['disk', 'diskUnlimited', 'Storage must be 0 or more MiB.']
  ])('validates %s once revealed', (key, sw, message) => {
    const revealed = { [sw]: false };
    expect(issues(resourcesStage, { ...revealed, [key]: 0 })).toEqual({});
    expect(issues(resourcesStage, { ...revealed, [key]: -1 })).toEqual({
      [key]: [message]
    });
    expect(issues(resourcesStage, { ...revealed, [key]: 1.5 })[key]).toEqual([
      expect.stringContaining('whole number')
    ]);
    expect(issues(resourcesStage, { ...revealed, [key]: null })[key]).toEqual([
      expect.stringContaining('is required')
    ]);
  });

  it.each([
    [-1, 'Backup limit must be 0 or more backups.'],
    [1.5, 'Backup limit must be a whole number.'],
    [null, 'Backup limit is required.']
  ])('rejects backup limit %j', (backupLimit, message) => {
    expect(issues(resourcesStage, { backupLimit }).backupLimit).toEqual([
      message
    ]);
  });
});

describe('startupStage stop timeout', () => {
  const valid = { image: 'img', startup: 'run' };

  it.each([1, 60, 3600])('accepts %j seconds', (stopTimeout) => {
    expect(issues(startupStage, { ...valid, stopTimeout }).stopTimeout).toBe(
      undefined
    );
  });

  it.each([
    [0, 'Stop timeout must be at least 1 second.'],
    [3601, 'Stop timeout must be at most 3600 seconds.'],
    [1.5, 'Stop timeout must be a whole number.'],
    [null, 'Stop timeout is required.']
  ])('rejects %j', (stopTimeout, message) => {
    expect(issues(startupStage, { ...valid, stopTimeout }).stopTimeout).toEqual(
      [message]
    );
  });
});

describe('permissionsStage', () => {
  const grant = (uuid: string[], access = ['read']) => ({ access, uuid });

  it('accepts distinct grants and the same uuid across lists', () => {
    expect(
      issues(permissionsStage, {
        groupGrants: [grant(['a'], ['write'])],
        userGrants: [grant(['a']), grant(['b'])]
      })
    ).toEqual({});
  });

  it('requires a target and an access level per list', () => {
    expect(
      issues(permissionsStage, {
        groupGrants: [grant([], [])],
        userGrants: [grant([], [])]
      })
    ).toEqual({
      'groupGrants.0.access': ['Select an access level.'],
      'groupGrants.0.uuid': ['Select a group.'],
      'userGrants.0.access': ['Select an access level.'],
      'userGrants.0.uuid': ['Select a user.']
    });
  });

  it('rejects an unknown access level', () => {
    expect(
      issues(permissionsStage, { userGrants: [grant(['a'], ['admin'])] })[
        'userGrants.0.access.0'
      ]
    ).toBeDefined();
  });

  it.each(['userGrants', 'groupGrants'])(
    'flags duplicates within %s',
    (key) => {
      expect(
        issues(permissionsStage, {
          [key]: [grant(['a']), grant(['b']), grant(['a'], ['write'])]
        })
      ).toEqual({ [`${key}.2.uuid`]: ['Already granted.'] });
    }
  );
});

describe('serverSettings', () => {
  it('accepts the converted server info', () => {
    expect(issues(serverSettings, base())).toEqual({});
  });

  // The backend sets owner NULL when the owning user is deleted.
  it('accepts a server without an owner (edit only)', () => {
    const f = fromServerInfo({ ...clone(serverInfo), owner: null });
    expect(f.owner).toEqual([]);
    expect(issues(serverSettings, f)).toEqual({});
    expect(toUpdateBody(f, false).owner).toBeNull();
  });

  it('rejects more than one owner', () => {
    const f = { ...base(), owner: ['a', 'b'] };
    expect(Object.keys(issues(serverSettings, f))).toEqual(['owner']);
  });

  it('runs every section refinement', () => {
    const f = base();
    f.allocations.push(clone(f.allocations[0]));
    f.variables[0].value = '';
    f.cpuUnlimited = false;
    f.cpu = -5;
    f.groupGrants.push(clone(f.groupGrants[0]));
    expect(Object.keys(issues(serverSettings, f)).toSorted()).toEqual([
      'allocations.2.port',
      'cpu',
      'groupGrants.1.uuid',
      'variables.0.value'
    ]);
  });
});

describe('optionsFromInfo', () => {
  it('lists only the server owner, node and egg', () => {
    expect(optionsFromInfo(serverInfo)).toEqual({
      eggs: [
        {
          docker_images: {},
          name: 'Paper',
          startup_commands: {},
          uuid: 'egg-paper',
          variables: []
        }
      ],
      groups: [],
      nodes: [{ name: 'eu-1', uuid: 'node-1' }],
      users: [{ name: 'Ada Admin', uuid: 'user-admin' }]
    });
  });

  it('has no users without an owner', () => {
    expect(optionsFromInfo({ ...serverInfo, owner: null }).users).toEqual([]);
  });
});

describe('isFieldErrors', () => {
  it.each([
    [{ errors: [] }, true],
    [{ errors: [{ field: 'ip', message: 'x' }] }, true],
    [{ errors: 'x' }, false],
    [{}, false],
    ['text', false],
    [null, false],
    [undefined, false]
  ])('%j -> %s', (body, expected) => {
    expect(isFieldErrors(body)).toBe(expected);
  });
});

describe('egg helpers', () => {
  it('finds an egg by uuid', () => {
    expect(findEgg(options, 'egg-paper')).toBe(paper);
    expect(findEgg(options, 'nope')).toBeUndefined();
    expect(findEgg(options)).toBeUndefined();
    expect(findEgg(undefined, 'egg-paper')).toBeUndefined();
  });

  it('uses the first command and image', () => {
    expect(defaultCommand(paper)).toBe('java -jar {{SERVER_JARFILE}}');
    expect(defaultCommand()).toBe('');
    expect(eggStartup(paper)).toEqual({
      image: 'ghcr.io/yolks:java_17',
      startup: 'java -jar {{SERVER_JARFILE}}',
      startupOverridden: false
    });
    expect(eggStartup()).toEqual({
      image: '',
      startup: '',
      startupOverridden: false
    });
  });

  it('builds variable rows in sort order holding the defaults', () => {
    expect(eggVariables(paper)).toEqual([
      {
        defaultValue: 'server.jar',
        description: 'Jar to run',
        editable: true,
        env: 'SERVER_JARFILE',
        name: 'Server Jar File',
        rules: ['required', 'string', 'max:20'],
        value: 'server.jar'
      },
      {
        defaultValue: '20',
        description: 'Player cap',
        editable: true,
        env: 'MAX_PLAYERS',
        name: 'Max players',
        rules: ['required', 'integer', 'between:1,100'],
        value: '20'
      }
    ]);
    expect(eggVariables()).toEqual([]);
  });

  it('keeps list order for unsorted variables and defaults missing rules', () => {
    const egg = {
      ...paper,
      variables: [
        { ...paper.variables[0], rules: undefined, sort: null },
        { ...paper.variables[1], sort: undefined }
      ]
    };
    const rows = eggVariables(egg);
    expect(rows.map((r) => r.env)).toEqual(['MAX_PLAYERS', 'SERVER_JARFILE']);
    expect(rows[0].rules).toEqual([]);
  });

  it('detects edited variables', () => {
    const rows = eggVariables(paper);
    expect(variablesEdited(rows)).toBe(false);
    expect(variablesEdited()).toBe(false);
    rows[1].value = '30';
    expect(variablesEdited(rows)).toBe(true);
  });
});

describe('converters', () => {
  it('converts server info to the form', () => {
    expect(base()).toEqual({
      allocations: serverInfo.allocations,
      backupLimit: 3,
      cpu: 2000,
      cpuUnlimited: false,
      disk: LIMIT_DEFAULTS.disk,
      diskUnlimited: true,
      egg: ['egg-paper'],
      groupGrants: [{ access: ['write'], uuid: ['group-admins'] }],
      image: 'ghcr.io/yolks:java_21',
      memory: 4096,
      memoryUnlimited: false,
      name: 'survival',
      node: ['node-1'],
      owner: ['user-admin'],
      restartOnCrash: false,
      runInstall: true,
      startOnCompletion: false,
      startup: 'java -jar custom.jar',
      startupOverridden: true,
      stopTimeout: 30,
      userGrants: [{ access: ['read'], uuid: ['user-1'] }],
      variables: [
        {
          defaultValue: 'server.jar',
          description: 'Jar to run',
          editable: true,
          env: 'SERVER_JARFILE',
          name: 'Server Jar File',
          rules: ['required', 'string', 'max:20'],
          value: 'paper.jar'
        },
        {
          defaultValue: '20',
          description: 'Player cap',
          editable: false,
          env: 'MAX_PLAYERS',
          name: 'Max players',
          rules: ['required', 'integer'],
          value: '10'
        }
      ]
    });
  });

  it('handles a missing owner and grants', () => {
    const f = fromServerInfo({
      ...clone(serverInfo),
      grants: null,
      owner: null
    });
    expect(f.owner).toEqual([]);
    expect(f.userGrants).toEqual([]);
    expect(f.groupGrants).toEqual([]);
    expect(fromGrantsInfo()).toEqual({ groupGrants: [], userGrants: [] });
  });

  it('builds the create body (unlimited sends 0, only editable variables)', () => {
    const f = base();
    f.name = '  survival  ';
    f.allocations[0].ip = ' 10.0.0.5 ';
    f.startup = ' run ';
    f.image = ' img ';
    f.runInstall = false;
    expect(toServerBody(f)).toEqual({
      allocations: [
        { ip: '10.0.0.5', port: 25_565 },
        { ip: '10.0.0.5', port: 25_566 }
      ],
      backup_limit: 3,
      egg: 'egg-paper',
      environment: { SERVER_JARFILE: 'paper.jar' },
      image: 'img',
      limits: { cpu: 2000, disk: 0, memory: 4096 },
      name: 'survival',
      node: 'node-1',
      owner: 'user-admin',
      restart_on_crash: false,
      skip_scripts: true,
      start_on_completion: false,
      startup: 'run',
      startup_overridden: true,
      stop_timeout: 30
    });
  });

  it('sends null without an owner and 0 for an empty revealed limit', () => {
    const f = base();
    f.owner = [];
    f.cpu = null;
    expect(toServerBody(f).owner).toBeNull();
    expect(toServerBody(f).limits.cpu).toBe(0);
  });

  it('builds the update body with or without grants', () => {
    const f = base();
    const withGrants = toUpdateBody(f, true);
    expect(withGrants.grants).toEqual({
      groups: [{ access: 'write', uuid: 'group-admins' }],
      users: [{ access: 'read', uuid: 'user-1' }]
    });
    const without = toUpdateBody(f, false);
    expect(without).not.toHaveProperty('grants');
    expect(without).not.toHaveProperty('node');
    expect(without).not.toHaveProperty('skip_scripts');
    expect(without).not.toHaveProperty('start_on_completion');
    expect(without.name).toBe('survival');
  });

  it('round-trips grants in order', () => {
    const rows: { userGrants: GrantRow[]; groupGrants: GrantRow[] } = {
      groupGrants: [{ access: ['read'], uuid: ['g'] }],
      userGrants: [
        { access: ['write'], uuid: ['b'] },
        { access: ['read'], uuid: ['a'] }
      ]
    };
    expect(toGrantsBody(rows)).toEqual({
      groups: [{ access: 'read', uuid: 'g' }],
      users: [
        { access: 'write', uuid: 'b' },
        { access: 'read', uuid: 'a' }
      ]
    });
    expect(
      fromGrantsInfo({
        groups: [{ access: 'read', name: 'G', uuid: 'g' }],
        users: [
          { access: 'write', name: 'B', uuid: 'b' },
          { access: 'read', name: 'A', uuid: 'a' }
        ]
      })
    ).toEqual(rows);
  });

  it('round-trips server info through the update body', () => {
    const f = base();
    const body = toUpdateBody(f, true);
    expect(body.allocations).toEqual(serverInfo.allocations);
    expect(body.limits).toEqual(serverInfo.limits);
    expect(body.startup).toBe(serverInfo.startup);
    expect(body.startup_overridden).toBe(serverInfo.startup_overridden);
    expect(body.image).toBe(serverInfo.image);
    expect(body.backup_limit).toBe(serverInfo.backup_limit);
  });
});

describe('mapFieldErrors', () => {
  const form = base();
  const body = (...fields: string[]) => ({
    errors: fields.map((field) => ({ field, message: `bad ${field}` }))
  });

  it.each([undefined, null, 'oops', {}, { errors: 'x' }])(
    'returns undefined for non FieldErrors %j',
    (b) => {
      expect(mapFieldErrors(b, form)).toBeUndefined();
    }
  );

  it.each([
    ['name', 'general', 'name'],
    ['owner', 'general', 'owner'],
    ['node', 'general', 'node'],
    ['egg', 'general', 'egg'],
    ['allocations', 'general', 'allocations._errors'],
    ['allocations.1', 'general', 'allocations.1.ip'],
    ['allocations.0.ip', 'general', 'allocations.0.ip'],
    ['allocations.2.port', 'general', 'allocations.2.port'],
    ['startup', 'startup', 'startup'],
    ['image', 'startup', 'image'],
    ['stop_timeout', 'startup', 'stopTimeout'],
    ['variables.MAX_PLAYERS', 'variables', 'variables.1.value'],
    ['limits.cpu', 'resources', 'cpu'],
    ['limits.memory', 'resources', 'memory'],
    ['limits.disk', 'resources', 'disk'],
    ['backup_limit', 'resources', 'backupLimit'],
    ['grants.users.0', 'permissions', 'userGrants.0.uuid'],
    ['grants.users.0.access', 'permissions', 'userGrants.0.uuid'],
    ['grants.groups.0', 'permissions', 'groupGrants.0.uuid']
  ])('maps %s to %s / %s', (field, section, path) => {
    expect(mapFieldErrors(body(field), form)).toEqual({
      first: section,
      other: [],
      sections: { [section]: { [path]: `bad ${field}` } }
    });
  });

  it.each([
    'variables.UNKNOWN',
    'grants.users.1',
    'grants.groups.1',
    'grants',
    'something',
    'allocations.x'
  ])('collects unmappable field %s as other', (field) => {
    expect(mapFieldErrors(body(field), form)).toEqual({
      first: undefined,
      other: [`bad ${field}`],
      sections: {}
    });
  });

  it('maps a backend overlap message onto the allocation row', () => {
    const message = 'Overlaps another allocation on the same port.';
    expect(
      mapFieldErrors({ errors: [{ field: 'allocations.1', message }] }, form)
    ).toEqual({
      first: 'general',
      other: [],
      sections: { general: { 'allocations.1.ip': message } }
    });
  });

  it('works without a form for direct fields', () => {
    expect(mapFieldErrors(body('name', 'variables.X'))).toEqual({
      first: 'general',
      other: ['bad variables.X'],
      sections: { general: { name: 'bad name' } }
    });
  });

  it('picks the first section in wizard order and keeps the first message per path', () => {
    const mapped = mapFieldErrors(
      {
        errors: [
          { field: 'limits.cpu', message: 'cpu' },
          { field: 'image', message: 'first' },
          { field: 'image', message: 'second' }
        ]
      },
      form
    );
    expect(mapped).toEqual({
      first: 'startup',
      other: [],
      sections: { resources: { cpu: 'cpu' }, startup: { image: 'first' } }
    });
  });
});
