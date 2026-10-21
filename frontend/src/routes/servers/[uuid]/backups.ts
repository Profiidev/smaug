import { z } from 'zod';
import type { BackupInfo, BackupStatus } from '$lib/client';
import { formatBytes } from '../format';
import type { BackupFrame } from './live.svelte';

const UNITS = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];

/** Matches the backend's name limit. */
export const NAME_MAX = 191;

/** `0.8 of 1.3 GiB`: both numbers in the unit of `total`. */
export const bytesOf = (bytes: number, total: number) => {
  let unit = 0;
  while (total >= 1024 ** (unit + 1) && unit < UNITS.length - 1) {
    unit += 1;
  }
  const fmt = (n: number) => (n / 1024 ** unit).toFixed(unit ? 1 : 0);
  return `${fmt(bytes)} of ${fmt(total)} ${UNITS[unit]}`;
};

/**
 * Progress of a creating/restoring backup. `total` is the expected size
 * (disk usage for creates, backup size for restores); unknown (0) shows bytes only.
 */
export const backupProgress = (
  bytes: number,
  total: number | undefined,
  verb: string
): { percent?: number; text: string } => {
  if (!total) {
    return { text: `${formatBytes(bytes)} ${verb}` };
  }
  const percent = Math.min(99, Math.floor((bytes / total) * 100));
  return { percent, text: `${percent}% · ${bytesOf(bytes, total)} ${verb}` };
};

/** `sha256 abcd…ef12`. */
export const shortHash = (hash: string) =>
  `sha256 ${hash.slice(0, 4)}…${hash.slice(-4)}`;

export const isBusy = (status: BackupStatus) =>
  status === 'creating' || status === 'restoring';

export type Progress = Record<string, { status: BackupStatus; bytes: number }>;

/**
 * Applies a `backup` frame to the live progress per backup id. A final frame
 * drops the entry; `reload` asks for a list refetch (final frame, or a backup
 * the list may not know yet).
 */
export const applyBackupFrame = (progress: Progress, frame: BackupFrame) => {
  const { [frame.id]: _, ...rest } = progress;
  if (!isBusy(frame.status)) {
    return { progress: rest, reload: true };
  }
  return {
    progress: {
      ...rest,
      [frame.id]: { bytes: frame.bytes, status: frame.status }
    },
    reload: !(frame.id in progress)
  };
};

/** A listed backup with its live progress overlaid. */
export const withProgress = (b: BackupInfo, progress: Progress) => ({
  ...b,
  bytes: progress[b.id]?.bytes ?? 0,
  status: progress[b.id]?.status ?? b.status
});

/** Default name the backend uses (UTC), shown as placeholder. */
export const defaultBackupName = (now = new Date()) =>
  `Backup at ${now.toISOString().slice(0, 16).replace('T', ' ')}`;

export const createBackupSchema = z.object({
  ignored: z.string().default(''),
  locked: z.boolean().default(false),
  name: z.string().trim().max(NAME_MAX, 'Name is too long').default('')
});

export const IGNORE_PLACEHOLDER = '*.log\ncache/';
