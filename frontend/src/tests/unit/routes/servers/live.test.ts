import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushSync } from 'svelte';
import { invalidate } from '$app/navigation';
import {
  MAX_LINES,
  POINTS,
  ServerLive
} from '$routes/servers/[uuid]/live.svelte';
import { FakeWebSocket, stubWebSocket } from '$test_helpers/fake-websocket';

const stats = {
  cpu_limit_millicores: 2000,
  cpu_millicores: 760,
  disk_bytes: 10,
  disk_limit_bytes: 100,
  memory_bytes: 2048,
  memory_limit_bytes: 4096,
  net_rx_bps: 14_000,
  net_tx_bps: 3000,
  type: 'stats',
  uptime_ms: 5000
} as const;

const socket = () => FakeWebSocket.last!;

let live = new ServerLive('srv-1');

beforeEach(() => {
  vi.useFakeTimers();
  stubWebSocket();
  live = new ServerLive('srv-1');
});

afterEach(() => {
  live.disconnect();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('ServerLive connection', () => {
  it('opens the server socket on the page origin', () => {
    live.connect();
    expect(socket().url).toBe(`ws://${location.host}/api/servers/srv-1/ws`);
    expect(live.connected).toBe(false);
    socket().open();
    expect(live.connected).toBe(true);
  });

  it('does not open a second socket while one is open', () => {
    live.connect();
    live.connect();
    expect(FakeWebSocket.all).toHaveLength(1);
  });

  it('reconnects with exponential backoff capped at 30 s', () => {
    live.connect();
    const delays = [1000, 2000, 4000, 8000, 16_000, 30_000, 30_000];
    for (const delay of delays) {
      const before = FakeWebSocket.all.length;
      socket().close();
      expect(live.connected).toBe(false);
      vi.advanceTimersByTime(delay - 1);
      expect(FakeWebSocket.all).toHaveLength(before);
      vi.advanceTimersByTime(1);
      expect(FakeWebSocket.all).toHaveLength(before + 1);
    }
  });

  it('resets the backoff once a socket stayed open for 5 s', () => {
    live.connect();
    socket().close();
    vi.advanceTimersByTime(1000);
    socket().close();
    vi.advanceTimersByTime(2000);
    socket().open();
    vi.advanceTimersByTime(5000);
    socket().close();
    vi.advanceTimersByTime(1000);
    expect(FakeWebSocket.all).toHaveLength(4);
  });

  it('keeps backing off when a socket closes right after opening', () => {
    live.connect();
    socket().open();
    socket().close();
    vi.advanceTimersByTime(1000);
    socket().open();
    vi.advanceTimersByTime(4999);
    socket().close();
    vi.advanceTimersByTime(1999);
    expect(FakeWebSocket.all).toHaveLength(2);
    vi.advanceTimersByTime(1);
    expect(FakeWebSocket.all).toHaveLength(3);
  });

  it('closes and stops reconnecting on disconnect', () => {
    live.connect();
    socket().open();
    live.disconnect();
    expect(socket().closed).toBe(true);
    expect(live.connected).toBe(false);
    vi.advanceTimersByTime(60_000);
    expect(FakeWebSocket.all).toHaveLength(1);
  });

  it('cancels a pending reconnect on disconnect', () => {
    live.connect();
    socket().close();
    live.disconnect();
    vi.advanceTimersByTime(60_000);
    expect(FakeWebSocket.all).toHaveLength(1);
  });
});

describe('ServerLive frames', () => {
  beforeEach(() => {
    live.connect();
    socket().open();
  });

  it('appends log lines with unique ids and dates', () => {
    socket().send({
      lines: [
        { at: '2026-01-01T12:00:00Z', line: 'hello', stream: 'console' },
        { at: '2026-01-01T12:00:01Z', line: 'marked', stream: 'daemon' }
      ],
      type: 'logs'
    });
    live.echo('list');
    expect(live.lines.map((l) => [l.stream, l.line])).toEqual([
      ['console', 'hello'],
      ['daemon', 'marked'],
      ['command', 'list']
    ]);
    expect(new Set(live.lines.map((l) => l.id)).size).toBe(3);
    expect(live.lines[0].at).toEqual(new Date('2026-01-01T12:00:00Z'));
  });

  it(`keeps only the last ${MAX_LINES} lines`, () => {
    const lines = Array.from({ length: MAX_LINES + 5 }, (_, i) => ({
      at: '2026-01-01T12:00:00Z',
      line: `l${i}`,
      stream: 'console'
    }));
    socket().send({ lines, type: 'logs' });
    expect(live.lines).toHaveLength(MAX_LINES);
    expect(live.lines[0].line).toBe('l5');
    expect(live.lines.at(-1)?.line).toBe(`l${MAX_LINES + 4}`);
  });

  it('tracks the status and reloads the server info', () => {
    socket().send({ status: 'running', type: 'state' });
    expect(live.status).toBe('running');
    expect(invalidate).toHaveBeenCalledWith('/api/servers/srv-1');
  });

  it('keeps the latest stats and a fixed-size series', () => {
    expect(live.cpu).toEqual(Array(POINTS).fill(0));
    socket().send(stats);
    socket().send({ ...stats, cpu_millicores: 800 });
    expect(live.stats?.cpu_millicores).toBe(800);
    expect(live.cpu).toHaveLength(POINTS);
    expect(live.cpu.slice(-2)).toEqual([760, 800]);
    expect(live.memory.at(-1)).toBe(2048);
    expect(live.rx.at(-1)).toBe(14_000);
    expect(live.tx.at(-1)).toBe(3000);
  });

  it('dispatches backup, schedule and op frames to listeners', () => {
    const backup = vi.fn();
    const schedule = vi.fn();
    const op = vi.fn();
    const off = live.on('backup', backup);
    live.on('schedule', schedule);
    live.on('op_finished', op);
    const frames = {
      backup: { bytes: 5, id: 'b1', status: 'creating', type: 'backup' },
      op: { error: null, ok: true, op: 'restore', type: 'op_finished' },
      schedule: {
        id: 's1',
        status: 'running',
        task_index: 0,
        type: 'schedule'
      }
    };
    socket().send(frames.backup);
    socket().send(frames.schedule);
    socket().send(frames.op);
    expect(backup).toHaveBeenCalledExactlyOnceWith(frames.backup);
    expect(schedule).toHaveBeenCalledExactlyOnceWith(frames.schedule);
    expect(op).toHaveBeenCalledExactlyOnceWith(frames.op);
    off();
    socket().send(frames.backup);
    expect(backup).toHaveBeenCalledOnce();
  });

  it('ignores malformed frames', () => {
    socket().send('not json');
    flushSync();
    expect(live.lines).toEqual([]);
    expect(live.connected).toBe(true);
  });
});
