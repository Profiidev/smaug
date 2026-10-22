import { invalidate } from '$app/navigation';
import type {
  LiveFrame,
  LiveStats,
  LogStream,
  ServerStatus
} from '$lib/client';

/** Lines kept client-side. */
export const MAX_LINES = 1000;
/** Chart points: 60 s at one stats frame every 2 s. */
export const POINTS = 30;
const MIN_DELAY = 1000;
const MAX_DELAY = 30_000;
/** Open this long before the backoff resets, so a flapping socket backs off. */
const STABLE = 5000;

export interface ConsoleLine {
  /** Unique per `ServerLive`, for keyed each blocks. */
  id: number;
  at: Date;
  /** `command` marks a locally echoed command. */
  stream: LogStream | 'command';
  line: string;
}

type Frame<T extends LiveFrame['type']> = Extract<LiveFrame, { type: T }>;
type EventType = 'backup' | 'schedule' | 'op_finished';
export type BackupFrame = Frame<'backup'>;
export type ScheduleFrame = Frame<'schedule'>;
export type OpFinishedFrame = Frame<'op_finished'>;

const zeros = () => Array<number>(POINTS).fill(0);
const push = (series: number[], value: number) => [...series.slice(1), value];

/**
 * Browser end of `GET /api/servers/{uuid}/ws`: console lines, status, metrics
 * and backup / schedule / op events. Owned by the server page, which connects
 * it while a live tab is open; reconnects with backoff while wanted.
 */
export class ServerLive {
  public connected = $state(false);
  /** Last status the node pushed; undefined until the first state frame. */
  public status: ServerStatus | undefined = $state();
  public lines: ConsoleLine[] = $state.raw([]);
  /** Latest stats frame. */
  public stats: LiveStats | undefined = $state.raw();
  /** Last `POINTS` samples, oldest first. CPU in millicores. */
  public cpu: number[] = $state.raw(zeros());
  public memory: number[] = $state.raw(zeros());
  public rx: number[] = $state.raw(zeros());
  public tx: number[] = $state.raw(zeros());

  public readonly uuid: string;
  readonly #listeners: { [T in EventType]: Set<(frame: Frame<T>) => void> } = {
    backup: new Set(),
    op_finished: new Set(),
    schedule: new Set()
  };
  #socket: WebSocket | undefined;
  #timer: ReturnType<typeof setTimeout> | undefined;
  #stable: ReturnType<typeof setTimeout> | undefined;
  #attempt = 0;
  #wanted = false;
  #nextId = 0;

  public constructor(uuid: string) {
    this.uuid = uuid;
  }

  public connect() {
    this.#wanted = true;
    clearTimeout(this.#timer);
    if (this.#socket) {
      return;
    }
    const proto = location.protocol === 'https:' ? 'wss' : 'ws';
    const ws = new WebSocket(
      `${proto}://${location.host}/api/servers/${this.uuid}/ws`
    );
    this.#socket = ws;
    ws.addEventListener('open', () => {
      this.connected = true;
      this.#stable = setTimeout(() => {
        this.#attempt = 0;
      }, STABLE);
    });
    ws.addEventListener('message', (e: MessageEvent<string>) => {
      try {
        const frame: LiveFrame = JSON.parse(e.data);
        this.handle(frame);
      } catch {
        // Ignore malformed frames.
      }
    });
    ws.addEventListener('close', () => {
      if (this.#socket !== ws) {
        return;
      }
      this.#socket = undefined;
      this.connected = false;
      clearTimeout(this.#stable);
      if (!this.#wanted) {
        return;
      }
      const delay = Math.min(MIN_DELAY * 2 ** this.#attempt, MAX_DELAY);
      this.#attempt += 1;
      this.#timer = setTimeout(() => this.connect(), delay);
    });
  }

  public disconnect() {
    this.#wanted = false;
    clearTimeout(this.#timer);
    clearTimeout(this.#stable);
    const ws = this.#socket;
    this.#socket = undefined;
    this.#attempt = 0;
    this.connected = false;
    ws?.close();
  }

  /** Subscribes to backup / schedule / op_finished frames; returns the unsubscribe. */
  public on<T extends EventType>(type: T, cb: (frame: Frame<T>) => void) {
    const set = this.#listeners[type] as Set<(frame: Frame<T>) => void>;
    set.add(cb);
    return () => {
      set.delete(cb);
    };
  }

  /** Shows a sent command in the console. */
  public echo(line: string) {
    this.#append([{ at: new Date(), line, stream: 'command' }]);
  }

  public handle(frame: LiveFrame) {
    switch (frame.type) {
      case 'logs': {
        this.#append(frame.lines.map((l) => ({ ...l, at: new Date(l.at) })));
        break;
      }
      case 'state': {
        this.status = frame.status;
        invalidate(`/api/servers/${this.uuid}`).catch(() => {});
        break;
      }
      case 'stats': {
        this.stats = frame;
        this.cpu = push(this.cpu, frame.cpu_millicores);
        this.memory = push(this.memory, frame.memory_bytes);
        this.rx = push(this.rx, frame.net_rx_bps);
        this.tx = push(this.tx, frame.net_tx_bps);
        break;
      }
      default: {
        this.#emit(frame.type, frame);
      }
    }
  }

  #emit<T extends EventType>(type: T, frame: Frame<T>) {
    for (const cb of this.#listeners[type]) {
      cb(frame);
    }
  }

  #append(lines: Omit<ConsoleLine, 'id'>[]) {
    const next = lines.map((l) => ({ ...l, id: (this.#nextId += 1) }));
    this.lines = [...this.lines, ...next].slice(-MAX_LINES);
  }
}
