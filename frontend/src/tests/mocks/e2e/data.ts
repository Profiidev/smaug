import { Permission } from '$lib/permissions.svelte';

/** Scenario name read from the `mock_scenario` cookie (set by the e2e tests). */
export type Scenario =
  | 'default'
  | 'empty'
  | 'readonly'
  | 'at-limit'
  | 'transfer-at-limit';

/** List data only varies between `default` and `empty`. */
export const scenarioOf = (
  cookies: Record<string, string>
): 'default' | 'empty' =>
  cookies.mock_scenario === 'empty' ? 'empty' : 'default';

/** Admin user with every permission, so admin pages render full controls. */
export const adminUser = {
  email: 'admin@example.com',
  name: 'Ada Admin',
  permissions: Object.values(Permission),
  totp_enabled: false,
  uuid: 'user-admin'
};

/**
 * `readonly` scenario: the admin minus `egg:edit`, `node:edit` and
 * `server:edit`, for read-only egg, node and server pages.
 */
export const userOf = (cookies: Record<string, string>) =>
  cookies.mock_scenario === 'readonly'
    ? {
        ...adminUser,
        permissions: adminUser.permissions.filter(
          (p) =>
            p !== Permission.EGG_EDIT &&
            p !== Permission.NODE_EDIT &&
            p !== Permission.SERVER_EDIT
        )
      }
    : adminUser;

const simpleUser = { id: 'user-1', name: 'Bob User' },
  simpleGroup = { name: 'Admins', uuid: 'group-admins' };

export const isSetup = {
  db_backend: 'sqlite',
  is_setup: true,
  storage_backend: 'local'
};

/** Returned when the `mock_setup=pending` cookie is set, so /setup renders. */
export const isSetupPending = {
  db_backend: 'sqlite',
  is_setup: false,
  storage_backend: 'local'
};

export const isSetupOf = (cookies: Record<string, string>) =>
  cookies.mock_setup === 'pending' ? isSetupPending : isSetup;

export const authConfig = { mail_enabled: true };
export const mailActive = { active: true };
/** `mock_mail=off` cookie disables mail, unlocking the admin-managed user
 * password/email/avatar controls that hide when mail is configured. */
export const mailActiveOf = (cookies: Record<string, string>) =>
  cookies.mock_mail === 'off' ? { active: false } : mailActive;
export const mailSettings = {
  from_env: [] as string[],
  settings: {
    smtp_enabled: false,
    smtp_from_name: 'Positron',
    smtp_use_tls: false
  }
};
export const siteUrl = { url: 'https://positron.example' };
export const oidcSettings = {
  client_id: '',
  issuer: '',
  scopes: [] as string[]
};

export const groups = {
  default: {
    admin_group: 'group-admins',
    groups: [
      {
        id: 'group-admins',
        name: 'Admins',
        permissions: [Permission.USER_VIEW, Permission.GROUP_VIEW],
        users: [simpleUser]
      },
      { id: 'group-staff', name: 'Staff', permissions: [], users: [] }
    ]
  },
  empty: { admin_group: undefined, groups: [] as unknown[] }
};

export const users = {
  default: [
    {
      email: 'bob@example.com',
      groups: [simpleGroup],
      name: 'Bob User',
      uuid: 'user-1'
    },
    {
      email: 'cara@example.com',
      groups: [],
      name: 'Cara User',
      uuid: 'user-2'
    }
  ],
  empty: [] as unknown[]
};

// Detail payloads.
export const groupDetails = {
  admin_group: 'group-admins',
  group: {
    id: 'group-admins',
    name: 'Admins',
    permissions: [Permission.USER_VIEW],
    users: [simpleUser]
  }
};

// A non-admin group, so the detail page renders the editable permissions
// Matrix (the admin group hides it).
export const groupStaffDetails = {
  admin_group: 'group-admins',
  group: {
    id: 'group-staff',
    name: 'Staff',
    permissions: [] as Permission[],
    users: [] as unknown[]
  }
};

export const userDetails = {
  email: 'bob@example.com',
  groups: [simpleGroup],
  name: 'Bob User',
  oidc_user: false,
  permissions: [Permission.USER_VIEW],
  uuid: 'user-1'
};

export const simpleUsers = { default: [simpleUser], empty: [] as unknown[] };
export const simpleGroups = { default: [simpleGroup], empty: [] as unknown[] };

// Eggs. `eggInfo` is the API shape (config as objects, no `meta.version`).
export const eggList = {
  default: {
    eggs: [
      {
        author: 'dev@example.com',
        description: 'Paper Minecraft server',
        has_update_url: true,
        name: 'Paper',
        uuid: 'egg-paper'
      },
      {
        author: 'ops@example.com',
        description: null,
        has_update_url: true,
        name: 'Valheim',
        uuid: 'egg-valheim'
      }
    ]
  },
  empty: { eggs: [] as unknown[] }
};

export const eggInfo = {
  author: 'dev@example.com',
  config: {
    files: {
      'server.properties': {
        find: { 'server-port': '{{server.allocations.default.port}}' },
        parser: 'properties'
      }
    },
    logs: {},
    startup: { done: ')! For help, type ' },
    stop: 'stop'
  },
  description: 'Paper Minecraft server',
  docker_images: { 'Java 21': 'ghcr.io/pelican-eggs/yolks:java_21' },
  features: ['eula'],
  file_denylist: [],
  icon: null,
  meta: { update_url: null },
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
  uuid: 'egg-paper',
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
};

// Nodes. `node-missing` answers 404 (see handlers).
export const nodeInfo = {
  address: 'node1.example.com',
  connected: true,
  cpu_limit: null,
  disk_limit_mb: null,
  id: 'node-1',
  memory_limit_mb: null,
  name: 'Node One',
  port: 443,
  secure: true
};

export const nodeList = {
  default: [
    nodeInfo,
    {
      address: '10.0.0.2',
      connected: false,
      cpu_limit: 2000,
      disk_limit_mb: 10_000,
      id: 'node-2',
      memory_limit_mb: 4096,
      name: 'Node Two',
      port: 8000,
      secure: false
    }
  ],
  empty: [] as unknown[]
};

export const generalSettings = { site_url: 'https://smaug.example/' };
export const userSettings = { from_env: [] as string[], settings: {} };

// Servers. `serverOptions` drives the create wizard; `srv-missing` answers 404.
const paperVariables = [
  {
    default_value: 'server.jar',
    description: 'Jar to run',
    env_variable: 'SERVER_JARFILE',
    name: 'Server Jar File',
    rules: ['required', 'string', 'max:20'],
    sort: 1
  },
  {
    default_value: '20',
    description: 'Player slots',
    env_variable: 'MAX_PLAYERS',
    name: 'Max Players',
    rules: ['required|integer|between:1,100'],
    sort: 2
  },
  {
    default_value: 'latest',
    description: '',
    env_variable: 'VERSION',
    name: 'Version',
    rules: ['nullable', 'in:latest,1.21'],
    sort: 3
  }
];

export const serverOptions = {
  default: {
    eggs: [
      {
        // oxlint-disable-next-line sort-keys -- order matters: first is the default
        docker_images: {
          'Java 21': 'ghcr.io/pelican-eggs/yolks:java_21',
          'Java 17': 'ghcr.io/pelican-eggs/yolks:java_17'
        },
        name: 'Paper',
        // oxlint-disable-next-line sort-keys -- order matters: first is the default
        startup_commands: {
          Default: 'java -jar {{SERVER_JARFILE}}',
          'Aikar flags': 'java -XX:+UseG1GC -jar {{SERVER_JARFILE}}'
        },
        uuid: 'egg-paper',
        variables: paperVariables
      },
      {
        docker_images: { Wine: 'ghcr.io/pelican-eggs/yolks:wine' },
        name: 'Valheim',
        startup_commands: { Default: './valheim_server' },
        uuid: 'egg-valheim',
        variables: [
          {
            default_value: 'My World',
            description: 'World name',
            env_variable: 'WORLD_NAME',
            name: 'World Name',
            rules: ['required', 'regex:/^[A-Za-z ]+$/']
          }
        ]
      },
      {
        docker_images: { Alpine: 'alpine:3' },
        name: 'Bare',
        startup_commands: { Default: './run' },
        uuid: 'egg-bare',
        variables: []
      }
    ],
    groups: [
      { name: 'Admins', uuid: 'group-admins' },
      { name: 'Staff', uuid: 'group-staff' }
    ],
    nodes: [
      { name: 'Node One', uuid: 'node-1' },
      { name: 'Node Two', uuid: 'node-2' }
    ],
    users: [
      { name: 'Ada Admin', uuid: 'user-admin' },
      { name: 'Bob User', uuid: 'user-1' }
    ]
  },
  empty: { eggs: [], groups: [], nodes: [], users: [] }
};

const paper = { name: 'Paper', uuid: 'egg-paper' },
  valheim = { name: 'Valheim', uuid: 'egg-valheim' },
  nodeOne = { name: 'Node One', uuid: 'node-1' },
  nodeTwo = { name: 'Node Two', uuid: 'node-2' },
  ada = { name: 'Ada Admin', uuid: 'user-admin' };

export const serverList = {
  default: {
    servers: [
      {
        egg: paper,
        extra_allocations: [
          { ip: '10.0.0.1', port: 25_566 },
          { ip: '10.0.0.1', port: 25_567 }
        ],
        name: 'Survival SMP',
        node: nodeOne,
        owner: ada,
        primary_allocation: { ip: '10.0.0.1', port: 25_565 },
        status: 'offline',
        uuid: 'srv-survival'
      },
      {
        egg: paper,
        extra_allocations: [],
        name: 'Creative',
        node: nodeTwo,
        owner: null,
        primary_allocation: { ip: '10.0.0.2', port: 25_570 },
        status: 'installing',
        uuid: 'srv-creative'
      },
      {
        egg: valheim,
        extra_allocations: [],
        name: 'Viking World',
        node: nodeOne,
        owner: ada,
        primary_allocation: { ip: '2001:db8::1', port: 2456 },
        status: 'install_failed',
        uuid: 'srv-viking'
      },
      {
        egg: paper,
        extra_allocations: [],
        name: 'Fresh Server',
        node: nodeTwo,
        owner: ada,
        primary_allocation: { ip: '10.0.0.2', port: 25_580 },
        status: 'not_installed',
        uuid: 'srv-fresh'
      },
      {
        egg: valheim,
        extra_allocations: [],
        name: 'Banned Realm',
        node: nodeOne,
        owner: ada,
        primary_allocation: { ip: '10.0.0.1', port: 2457 },
        status: 'suspended',
        uuid: 'srv-banned'
      }
    ]
  },
  empty: { servers: [] as unknown[] }
};

const installState = {
  install_failed: 'install_failed',
  installing: 'installing',
  not_installed: 'not_installed'
} as Record<string, string>;

/**
 * `GET /servers/{uuid}` payload for a listed server as seen with `access`
 * (`admin` also gets grants). Unknown uuids fall back to the first server.
 */
export const serverInfoOf = (
  uuid: string,
  access: 'read' | 'write' | 'admin' = 'admin'
) => {
  const item =
    serverList.default.servers.find((s) => s.uuid === uuid) ??
    serverList.default.servers[0];
  const egg = serverOptions.default.eggs.find((e) => e.uuid === item.egg.uuid)!;
  return {
    access,
    allocations: [item.primary_allocation, ...item.extra_allocations],
    backup_limit: 2,
    created_at: '2026-01-01T00:00:00Z',
    egg: item.egg,
    grants:
      access === 'admin'
        ? {
            groups: [{ access: 'read', name: 'Staff', uuid: 'group-staff' }],
            users: [{ access: 'write', name: 'Bob User', uuid: 'user-1' }]
          }
        : null,
    image: Object.values(egg.docker_images)[0],
    install_state: installState[item.status] ?? 'installed',
    limits: { cpu: 2000, disk: 0, memory: 4096 },
    name: item.name,
    node: item.node,
    owner: item.owner,
    start_on_completion: true,
    startup: Object.values(egg.startup_commands)[0],
    startup_overridden: false,
    status: item.status,
    suspended: item.status === 'suspended',
    uuid: item.uuid,
    variables: egg.variables
      .filter((_, i) => access === 'admin' || i < 2)
      .map((v) => ({
        default_value: v.default_value,
        description: v.description,
        editable: access === 'admin' || v.env_variable !== 'SERVER_JARFILE',
        env_variable: v.env_variable,
        name: v.name,
        rules: v.rules,
        value: v.default_value
      }))
  };
};

/** `readonly` scenario only reads servers; everyone else is admin. */
export const serverAccessOf = (cookies: Record<string, string>) =>
  cookies.mock_scenario === 'readonly' ? 'read' : 'admin';
