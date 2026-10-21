import { z } from 'zod';
import type {
  CronFields,
  FieldErrors,
  ScheduleAction,
  ScheduleInfo,
  ScheduleStatus,
  TaskInfo
} from '$lib/client';
import type { ScheduleFrame } from './live.svelte';
import { NAME_MAX } from './backups';

export const CRON_FIELDS = [
  ['minute', 'Minute'],
  ['hour', 'Hour'],
  ['day_of_month', 'Day of month'],
  ['month', 'Month'],
  ['day_of_week', 'Day of week']
] as const satisfies [keyof CronFields, string][];

const DAYS = [
  'Sunday',
  'Monday',
  'Tuesday',
  'Wednesday',
  'Thursday',
  'Friday',
  'Saturday'
];
const MONTHS = [
  'Jan',
  'Feb',
  'Mar',
  'Apr',
  'May',
  'Jun',
  'Jul',
  'Aug',
  'Sep',
  'Oct',
  'Nov',
  'Dec'
];

const num = (v: string) => (/^\d+$/.test(v) ? Number(v) : undefined);
const every = (v: string) => /^\*\/(?<n>\d+)$/.exec(v)?.groups?.n;
const pad = (n: number) => String(n).padStart(2, '0');

export const cronString = (c: CronFields) =>
  `${c.minute} ${c.hour} ${c.day_of_month} ${c.month} ${c.day_of_week}`;

/** `Every 5 minutes`, `Every hour at minute 3`, `Every 2 hours` when day fields are `*`. */
const intervalText = (minute: string, hour: string, m: number | undefined) => {
  if (hour === '*') {
    if (minute === '*') {
      return 'Every minute';
    }
    const n = every(minute);
    if (n) {
      return n === '1' ? 'Every minute' : `Every ${n} minutes`;
    }
    if (m === 0) {
      return 'Every hour';
    }
    if (m !== undefined) {
      return `Every hour at minute ${m}`;
    }
  }
  const n = every(hour);
  if (n && m !== undefined) {
    return `Every ${n} hours${m ? ` at minute ${m}` : ''}`;
  }
  return undefined;
};

/** Plain-English cron for common shapes; the raw expression otherwise. */
export const cronText = (c: CronFields) => {
  const { minute, hour, day_of_month: dom, month, day_of_week: dow } = c;
  const m = num(minute);
  const h = num(hour);
  const d = num(dom);
  const w = num(dow);
  if (month !== '*') {
    return cronString(c);
  }
  if (dom === '*' && dow === '*') {
    const interval = intervalText(minute, hour, m);
    if (interval) {
      return interval;
    }
  }
  if (m === undefined || h === undefined) {
    return cronString(c);
  }
  const at = `${pad(h)}:${pad(m)}`;
  if (dom === '*' && dow === '*') {
    return `Every day at ${at}`;
  }
  if (dom === '*' && w !== undefined && w <= 7) {
    return `Every ${DAYS[w % 7]} at ${at}`;
  }
  if (dow === '*' && d !== undefined) {
    return `On day ${d} of the month at ${at}`;
  }
  return cronString(c);
};

const DAY_MS = 86_400_000;
const utcDay = (d: Date) => Math.floor(d.getTime() / DAY_MS);

/** `Today, 04:00`, `Tomorrow, 04:00`, `Sun, 03:00`, `Oct 1, 02:30`; UTC like the cron. */
export const formatRunTime = (d: Date, now = new Date()) => {
  const at = d.toISOString().slice(11, 16);
  const diff = utcDay(d) - utcDay(now);
  if (diff === 0) {
    return `Today, ${at}`;
  }
  if (diff === 1) {
    return `Tomorrow, ${at}`;
  }
  if (diff === -1) {
    return `Yesterday, ${at}`;
  }
  if (Math.abs(diff) < 7) {
    return `${DAYS[d.getUTCDay()].slice(0, 3)}, ${at}`;
  }
  return `${MONTHS[d.getUTCMonth()]} ${d.getUTCDate()}, ${at}`;
};

export const lastRunText = (s: ScheduleInfo) =>
  s.last_run_at ? formatRunTime(s.last_run_at) : 'Never';

export const nextRunText = (s: ScheduleInfo) => {
  if (s.status === 'running') {
    return 'Running now';
  }
  return s.next_run_at ? formatRunTime(s.next_run_at) : 'Paused';
};

export const STATUS_LABELS: Record<ScheduleStatus, string> = {
  active: 'Active',
  inactive: 'Inactive',
  running: 'Running'
};

export const ACTION_LABELS: Record<ScheduleAction, string> = {
  backup: 'Create backup',
  command: 'Send command',
  power: 'Send power action'
};

export const POWER_ACTIONS = ['start', 'stop', 'restart', 'kill'] as const;

export const offsetText = (t: Pick<TaskInfo, 'offset_seconds'>) =>
  t.offset_seconds ? `After ${t.offset_seconds} s` : 'Runs right away';

/** Running task index per schedule id, from `schedule` frames. */
export type Runs = Record<string, number | undefined>;

/** Applies a frame; `reload` is set once a run finished. */
export const applyScheduleFrame = (runs: Runs, frame: ScheduleFrame) => {
  const { [frame.id]: _, ...rest } = runs;
  return frame.status === 'running'
    ? {
        reload: false,
        runs: { ...rest, [frame.id]: frame.task_index ?? undefined }
      }
    : { reload: true, runs: rest };
};

/** A schedule with its live run state overlaid. */
export const withRun = (s: ScheduleInfo, runs: Runs): ScheduleInfo =>
  s.id in runs ? { ...s, running_task: runs[s.id], status: 'running' } : s;

export const scheduleSchema = z.object({
  day_of_month: z.string().trim().min(1, 'Required').default('*'),
  day_of_week: z.string().trim().min(1, 'Required').default('*'),
  enabled: z.boolean().default(true),
  hour: z.string().trim().min(1, 'Required').default('*'),
  minute: z.string().trim().min(1, 'Required').default('0'),
  month: z.string().trim().min(1, 'Required').default('*'),
  name: z
    .string()
    .trim()
    .min(1, 'Name is required')
    .max(NAME_MAX, 'Name is too long')
    .default(''),
  only_when_online: z.boolean().default(false)
});

export type ScheduleForm = z.infer<typeof scheduleSchema>;

export const scheduleForm = (s?: ScheduleInfo): ScheduleForm => ({
  ...(s?.cron ?? {
    day_of_month: '*',
    day_of_week: '*',
    hour: '*',
    minute: '0',
    month: '*'
  }),
  enabled: s?.enabled ?? true,
  name: s?.name ?? '',
  only_when_online: s?.only_when_online ?? false
});

export const scheduleBody = (f: ScheduleForm) => ({
  cron: {
    day_of_month: f.day_of_month,
    day_of_week: f.day_of_week,
    hour: f.hour,
    minute: f.minute,
    month: f.month
  },
  enabled: f.enabled,
  name: f.name,
  only_when_online: f.only_when_online
});

export const taskSchema = z
  .object({
    action: z.enum(['command', 'power', 'backup']).default('command'),
    continue_on_failure: z.boolean().default(false),
    offset_seconds: z
      .number({ error: 'Offset is required' })
      .int('Whole seconds only')
      .min(0, 'Offset can not be negative')
      .default(0),
    payload: z.string().default('')
  })
  .superRefine((t, ctx) => {
    if (t.action === 'command' && !t.payload.trim()) {
      ctx.addIssue({
        code: 'custom',
        message: 'Command is required',
        path: ['payload']
      });
    }
    if (
      t.action === 'power' &&
      !(POWER_ACTIONS as readonly string[]).includes(t.payload.trim())
    ) {
      ctx.addIssue({
        code: 'custom',
        message: 'Pick start, stop, restart or kill',
        path: ['payload']
      });
    }
  });

export type TaskForm = z.infer<typeof taskSchema>;

const isFieldErrors = (body: unknown): body is FieldErrors =>
  body instanceof Object && 'errors' in body && Array.isArray(body.errors);

/**
 * First error of a 400 `FieldErrors` body as a form error; `cron.minute`
 * maps to `minute`, `action` to `payload`. Undefined when not such a body.
 */
export const fieldError = <F extends string>(
  body: unknown,
  fields: readonly F[]
): { error: string; field?: F } | undefined => {
  const first = isFieldErrors(body) ? body.errors[0] : undefined;
  if (!first) {
    return undefined;
  }
  const name = first.field
    .replace(/^cron\./, '')
    .replace(/^action$/, 'payload');
  const field = fields.find((f) => f === name);
  return { error: first.message, field };
};
