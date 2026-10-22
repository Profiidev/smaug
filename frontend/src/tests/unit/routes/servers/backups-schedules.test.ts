import { describe, expect, it } from 'vitest';
import type { BackupInfo, CronFields, ScheduleInfo } from '$lib/client';
import {
  applyBackupFrame,
  backupProgress,
  bytesOf,
  createBackupSchema,
  defaultBackupName,
  pruneProgress,
  restoreFailed,
  shortHash,
  withProgress
} from '$routes/servers/[uuid]/backups';
import {
  applyScheduleFrame,
  cronText,
  fieldError,
  formatRunTime,
  nextRunText,
  offsetText,
  pruneRuns,
  scheduleBody,
  scheduleForm,
  scheduleSchema,
  taskSchema,
  withRun
} from '$routes/servers/[uuid]/schedules';

const GiB = 1024 ** 3;

const cron = (expr: string): CronFields => {
  const [minute, hour, day_of_month, month, day_of_week] = expr.split(' ');
  return { day_of_month, day_of_week, hour, minute, month };
};

const schedule = (over: Partial<ScheduleInfo> = {}): ScheduleInfo => ({
  created_at: new Date(0),
  cron: cron('0 4 * * *'),
  enabled: true,
  id: 's1',
  name: 'Nightly',
  only_when_online: false,
  status: 'active',
  tasks: [],
  ...over
});

describe('cronText', () => {
  it.each([
    ['* * * * *', 'Every minute'],
    ['*/1 * * * *', 'Every minute'],
    ['*/5 * * * *', 'Every 5 minutes'],
    ['0 * * * *', 'Every hour'],
    ['30 * * * *', 'Every hour at minute 30'],
    ['0 */6 * * *', 'Every 6 hours'],
    ['15 */2 * * *', 'Every 2 hours at minute 15'],
    ['0 4 * * *', 'Every day at 04:00'],
    ['5 23 * * *', 'Every day at 23:05'],
    ['0 3 * * 0', 'Every Sunday at 03:00'],
    ['0 3 * * 7', 'Every Sunday at 03:00'],
    ['0 3 * * 1', 'Every Monday at 03:00'],
    ['30 2 1 * *', 'On day 1 of the month at 02:30'],
    // Fallback: raw expression.
    ['0 4 * 1 *', '0 4 * 1 *'],
    ['0 4 * * 1-5', '0 4 * * 1-5'],
    ['0 4 1 * 1', '0 4 1 * 1'],
    ['1,15 4 * * *', '1,15 4 * * *'],
    ['0 4 * * 9', '0 4 * * 9']
  ])('%s -> %s', (expr, text) => {
    expect(cronText(cron(expr))).toBe(text);
  });
});

describe('formatRunTime', () => {
  const now = new Date('2026-10-21T12:00:00Z');
  it.each([
    ['2026-10-21T04:00:00Z', 'Today, 04:00'],
    ['2026-10-22T04:00:00Z', 'Tomorrow, 04:00'],
    ['2026-10-20T23:59:00Z', 'Yesterday, 23:59'],
    ['2026-10-25T03:00:00Z', 'Sun, 03:00'],
    ['2026-10-01T02:30:00Z', 'Oct 1, 02:30']
  ])('%s -> %s', (at, text) => {
    expect(formatRunTime(new Date(at), now)).toBe(text);
  });
});

describe('schedule texts', () => {
  it('next run reads Running now while running and Paused without one', () => {
    expect(nextRunText(schedule({ status: 'running' }))).toBe('Running now');
    expect(nextRunText(schedule({ next_run_at: null }))).toBe('Paused');
  });

  it('offset text', () => {
    expect(offsetText({ offset_seconds: 0 })).toBe('Runs right away');
    expect(offsetText({ offset_seconds: 300 })).toBe('After 300 s');
  });
});

describe('schedule frames', () => {
  it('marks a schedule running with its task and clears it when idle', () => {
    const running = applyScheduleFrame(
      {},
      { id: 's1', status: 'running', task_index: 1, type: 'schedule' }
    );
    expect(running).toEqual({ reload: false, runs: { s1: 1 } });
    const s = withRun(schedule(), running.runs);
    expect(s.status).toBe('running');
    expect(s.running_task).toBe(1);

    const idle = applyScheduleFrame(running.runs, {
      id: 's1',
      status: 'idle',
      type: 'schedule'
    });
    expect(idle).toEqual({ reload: true, runs: {} });
    expect(withRun(schedule(), idle.runs).status).toBe('active');
  });

  it('keeps other schedules running', () => {
    const { runs } = applyScheduleFrame(
      { s2: 0 },
      { id: 's1', status: 'running', task_index: null, type: 'schedule' }
    );
    expect(runs).toEqual({ s1: undefined, s2: 0 });
    expect('s1' in runs).toBe(true);
  });

  it('prunes runs the fetched list has no longer running', () => {
    const list = [
      schedule({ id: 's1', status: 'running' }),
      schedule({ id: 's2' })
    ];
    expect(pruneRuns({ s1: 0, s2: 1, s3: undefined }, list)).toEqual({
      s1: 0
    });
  });
});

describe('schedule forms', () => {
  it('round-trips a schedule', () => {
    const s = schedule({ only_when_online: true });
    expect(scheduleBody(scheduleForm(s))).toEqual({
      cron: s.cron,
      enabled: true,
      name: 'Nightly',
      only_when_online: true
    });
  });

  it('defaults a new schedule to hourly, enabled', () => {
    const f = scheduleForm();
    expect(cronText(scheduleBody(f).cron)).toBe('Every hour');
    expect(f.enabled).toBe(true);
    expect(scheduleSchema.safeParse(f).success).toBe(false); // Name empty
  });

  it('rejects empty cron fields and overly long names', () => {
    const f = scheduleForm(schedule());
    expect(scheduleSchema.safeParse(f).success).toBe(true);
    expect(scheduleSchema.safeParse({ ...f, minute: ' ' }).success).toBe(false);
    expect(
      scheduleSchema.safeParse({ ...f, name: 'x'.repeat(192) }).success
    ).toBe(false);
  });

  it.each([
    [{ action: 'command', payload: 'say hi' }, true],
    [{ action: 'command', payload: '  ' }, false],
    [{ action: 'power', payload: 'restart' }, true],
    [{ action: 'power', payload: 'explode' }, false],
    [{ action: 'backup', payload: '' }, true],
    [{ action: 'command', offset_seconds: -1, payload: 'x' }, false],
    [{ action: 'command', offset_seconds: 1.5, payload: 'x' }, false]
  ])('task %o valid=%s', (task, valid) => {
    expect(
      taskSchema.safeParse({
        continue_on_failure: false,
        offset_seconds: 0,
        ...task
      }).success
    ).toBe(valid);
  });
});

describe('fieldError', () => {
  const fields = ['name', 'minute', 'payload'] as const;

  it('maps cron.* and action to form fields', () => {
    expect(
      fieldError(
        { errors: [{ field: 'cron.minute', message: 'Bad minute' }] },
        fields
      )
    ).toEqual({ error: 'Bad minute', field: 'minute' });
    expect(
      fieldError({ errors: [{ field: 'action', message: 'No' }] }, fields)
    ).toEqual({ error: 'No', field: 'payload' });
  });

  it('keeps unknown fields as a general error', () => {
    expect(
      fieldError({ errors: [{ field: 'other', message: 'Oops' }] }, fields)
    ).toEqual({ error: 'Oops', field: undefined });
  });

  it('ignores non FieldErrors bodies', () => {
    expect(fieldError('nope', fields)).toBeUndefined();
    expect(fieldError(undefined, fields)).toBeUndefined();
    expect(fieldError({ errors: [] }, fields)).toBeUndefined();
  });
});

describe('backup progress', () => {
  it('shares the unit of the total', () => {
    expect(bytesOf(0.8 * GiB, 1.3 * GiB)).toBe('0.8 of 1.3 GiB');
    expect(bytesOf(10, 500)).toBe('10 of 500 B');
  });

  it('shows percent of a known total', () => {
    expect(backupProgress(0.806 * GiB, 1.3 * GiB, 'archived')).toEqual({
      percent: 62,
      text: '62% · 0.8 of 1.3 GiB archived'
    });
  });

  it('never claims 100% before the final frame', () => {
    expect(backupProgress(2 * GiB, GiB, 'archived').percent).toBe(99);
  });

  it('shows bytes only without a total', () => {
    expect(backupProgress(512 * 1024 ** 2, 0, 'archived')).toEqual({
      text: '512 MiB archived'
    });
    expect(backupProgress(10, undefined, 'restored').percent).toBeUndefined();
  });

  it('shortens hashes', () => {
    expect(shortHash(`a1f3${'0'.repeat(56)}9c2e`)).toBe('sha256 a1f3…9c2e');
  });

  it('uses the backend default name format', () => {
    expect(defaultBackupName(new Date('2026-10-21T12:30:45Z'))).toBe(
      'Backup at 2026-10-21 12:30'
    );
  });

  it('accepts an empty create form', () => {
    expect(createBackupSchema.safeParse({}).success).toBe(true);
    expect(
      createBackupSchema.safeParse({ name: 'x'.repeat(192) }).success
    ).toBe(false);
  });
});

describe('backup frames', () => {
  const backup: BackupInfo = {
    created_at: new Date(0),
    id: 'b1',
    ignored: '',
    locked: false,
    name: 'B',
    size_bytes: 100,
    status: 'completed'
  };

  it('tracks progress, reloading on the first and final frame', () => {
    const first = applyBackupFrame(
      {},
      { bytes: 10, id: 'b1', status: 'creating', type: 'backup' }
    );
    expect(first).toEqual({
      progress: { b1: { bytes: 10, status: 'creating' } },
      reload: true
    });
    const second = applyBackupFrame(first.progress, {
      bytes: 20,
      id: 'b1',
      status: 'creating',
      type: 'backup'
    });
    expect(second.reload).toBe(false);
    expect(withProgress(backup, second.progress)).toMatchObject({
      bytes: 20,
      status: 'creating'
    });

    const done = applyBackupFrame(second.progress, {
      bytes: 30,
      id: 'b1',
      status: 'completed',
      type: 'backup'
    });
    expect(done).toEqual({ progress: {}, reload: true });
    expect(withProgress(backup, done.progress).status).toBe('completed');
  });

  it('prunes progress the list no longer has busy', () => {
    const list = [
      { ...backup, id: 'b1', status: 'creating' as const },
      { ...backup, id: 'b2' }
    ];
    expect(
      pruneProgress(
        {
          b1: { bytes: 1, status: 'creating' },
          b2: { bytes: 2, status: 'restoring' },
          b3: { bytes: 3, status: 'creating' }
        },
        list
      )
    ).toEqual({ b1: { bytes: 1, status: 'creating' } });
  });

  it('treats failed as final', () => {
    expect(
      applyBackupFrame(
        { b1: { bytes: 1, status: 'restoring' } },
        { bytes: 1, id: 'b1', status: 'failed', type: 'backup' }
      )
    ).toEqual({ progress: {}, reload: true });
  });

  it('flags only a failed restore, the list resets it to completed', () => {
    const failed = {
      bytes: 0,
      id: 'b1',
      status: 'failed',
      type: 'backup'
    } as const;
    expect(
      restoreFailed({ b1: { bytes: 5, status: 'restoring' } }, failed)
    ).toBe(true);
    // A failed create stays failed in the list
    expect(
      restoreFailed({ b1: { bytes: 5, status: 'creating' } }, failed)
    ).toBe(false);
    expect(restoreFailed({}, failed)).toBe(false);
  });
});
