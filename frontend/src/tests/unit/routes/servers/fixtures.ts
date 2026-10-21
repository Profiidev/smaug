import type { ServerInfo, ServerListItem, ServerOptions } from '$lib/client';

export const options: ServerOptions = {
  eggs: [
    {
      docker_images: {
        'Java 17': 'ghcr.io/yolks:java_17',
        'Java 21': 'ghcr.io/yolks:java_21'
      },
      name: 'Paper',
      // oxlint-disable-next-line eslint/sort-keys -- first command is the default
      startup_commands: {
        Default: 'java -jar {{SERVER_JARFILE}}',
        Aikar: 'java -XX:+UseG1GC -jar {{SERVER_JARFILE}}'
      },
      uuid: 'egg-paper',
      variables: [
        {
          default_value: '20',
          description: 'Player cap',
          env_variable: 'MAX_PLAYERS',
          name: 'Max players',
          rules: ['required', 'integer', 'between:1,100'],
          sort: 2
        },
        {
          default_value: 'server.jar',
          description: 'Jar to run',
          env_variable: 'SERVER_JARFILE',
          name: 'Server Jar File',
          rules: ['required', 'string', 'max:20'],
          sort: 1
        }
      ]
    },
    {
      docker_images: { Alpine: 'alpine:3' },
      name: 'Vanilla',
      startup_commands: { Default: './run.sh' },
      uuid: 'egg-vanilla',
      variables: []
    }
  ],
  groups: [{ name: 'Admins', uuid: 'group-admins' }],
  nodes: [
    { name: 'eu-1', uuid: 'node-1' },
    { name: 'us-1', uuid: 'node-2' }
  ],
  users: [
    { name: 'Ada Admin', uuid: 'user-admin' },
    { name: 'Bob User', uuid: 'user-1' }
  ]
};

export const serverInfo: ServerInfo = {
  access: 'admin',
  allocations: [
    { ip: '10.0.0.5', port: 25_565 },
    { ip: '10.0.0.5', port: 25_566 }
  ],
  backup_limit: 3,
  created_at: new Date('2026-01-01T00:00:00Z'),
  egg: { name: 'Paper', uuid: 'egg-paper' },
  grants: {
    groups: [{ access: 'write', name: 'Admins', uuid: 'group-admins' }],
    users: [{ access: 'read', name: 'Bob User', uuid: 'user-1' }]
  },
  image: 'ghcr.io/yolks:java_21',
  install_state: 'installed',
  limits: { cpu: 2000, disk: 0, memory: 4096 },
  name: 'survival',
  node: { name: 'eu-1', uuid: 'node-1' },
  owner: { name: 'Ada Admin', uuid: 'user-admin' },
  start_on_completion: false,
  startup: 'java -jar custom.jar',
  startup_overridden: true,
  status: 'offline',
  suspended: false,
  uuid: 'srv-1',
  variables: [
    {
      default_value: 'server.jar',
      description: 'Jar to run',
      editable: true,
      env_variable: 'SERVER_JARFILE',
      name: 'Server Jar File',
      rules: ['required', 'string', 'max:20'],
      value: 'paper.jar'
    },
    {
      default_value: '20',
      description: 'Player cap',
      editable: false,
      env_variable: 'MAX_PLAYERS',
      name: 'Max players',
      rules: ['required', 'integer'],
      value: '10'
    }
  ]
};

export const servers: ServerListItem[] = [
  {
    egg: { name: 'Paper', uuid: 'egg-paper' },
    extra_allocations: [{ ip: '10.0.0.5', port: 25_566 }],
    name: 'survival',
    node: { name: 'eu-1', uuid: 'node-1' },
    owner: { name: 'Ada Admin', uuid: 'user-admin' },
    primary_allocation: { ip: '10.0.0.5', port: 25_565 },
    status: 'offline',
    uuid: 'srv-1'
  },
  {
    egg: { name: 'Vanilla', uuid: 'egg-vanilla' },
    extra_allocations: [],
    name: 'creative',
    node: { name: 'us-1', uuid: 'node-2' },
    owner: null,
    primary_allocation: { ip: '::1', port: 30_000 },
    status: 'installing',
    uuid: 'srv-2'
  }
];
