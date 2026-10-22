import type { PowerAction } from '$lib/client';

/** Server lifecycle states, matching the backend's snake_case status. */
export type ServerStatus =
  | 'running'
  | 'starting'
  | 'stopping'
  | 'crashed'
  | 'offline'
  | 'unknown'
  | 'installing'
  | 'install_failed'
  | 'not_installed'
  | 'suspended'
  | 'transferring';

/** Badge label and dot colour (CSS) per status; dots are the only custom colour. */
export const STATUS: Record<ServerStatus, { label: string; color: string }> = {
  crashed: { color: 'oklch(0.65 0.22 25)', label: 'Crashed' },
  install_failed: { color: 'oklch(0.65 0.22 25)', label: 'Install failed' },
  installing: { color: 'oklch(0.70 0.14 250)', label: 'Installing' },
  not_installed: { color: 'oklch(0.60 0 0)', label: 'Not installed' },
  offline: { color: 'oklch(0.60 0 0)', label: 'Offline' },
  running: { color: 'oklch(0.72 0.17 150)', label: 'Running' },
  starting: { color: 'oklch(0.80 0.15 85)', label: 'Starting' },
  stopping: { color: 'oklch(0.80 0.15 85)', label: 'Stopping' },
  suspended: { color: 'oklch(0.75 0.16 55)', label: 'Suspended' },
  transferring: { color: 'oklch(0.70 0.14 250)', label: 'Transferring' },
  unknown: { color: 'oklch(0.60 0 0)', label: 'Unknown' }
};

export interface ServerActions {
  reinstall: boolean;
  /** Mark as installed / not installed. */
  install: boolean;
  /** Suspend, or Unsuspend while suspended. */
  suspend: boolean;
  transfer: boolean;
  delete: boolean;
}

const IDLE = new Set<ServerStatus>([
  'running',
  'starting',
  'stopping',
  'crashed',
  'offline',
  'unknown',
  'install_failed',
  'not_installed'
]);

/** Which actions the status allows (handoff matrix; the backend answers 409 otherwise). */
export const availableActions = (status: ServerStatus): ServerActions => {
  const idle = IDLE.has(status);
  const installing = status === 'installing';
  const suspended = status === 'suspended';
  return {
    delete: idle || suspended,
    install: idle || installing || suspended,
    reinstall: idle,
    suspend: idle || installing || suspended,
    transfer: idle
  };
};

/** Power actions the status allows (handoff power matrix; the backend answers 409 otherwise). */
export const powerActions = (status: ServerStatus): PowerAction[] => {
  switch (status) {
    case 'running': {
      return ['restart', 'stop', 'kill'];
    }
    case 'starting': {
      return ['stop', 'kill'];
    }
    case 'stopping': {
      return ['kill'];
    }
    case 'offline':
    case 'crashed': {
      return ['start'];
    }
    default: {
      return [];
    }
  }
};
