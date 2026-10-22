import { vi } from 'vitest';

/** Minimal `WebSocket` stand-in; every instance lands in `FakeWebSocket.all`. */
export class FakeWebSocket extends EventTarget {
  static all: FakeWebSocket[] = [];
  readonly url: string;
  closed = false;

  constructor(url: string) {
    super();
    this.url = url;
    FakeWebSocket.all.push(this);
  }

  /** Server side: the handshake finished. */
  open() {
    this.dispatchEvent(new Event('open'));
  }

  /** Server side: pushes one JSON frame (or raw text). */
  send(frame: unknown) {
    this.dispatchEvent(
      new MessageEvent('message', {
        data: typeof frame === 'string' ? frame : JSON.stringify(frame)
      })
    );
  }

  /** Client or server side close; fires `close` once. */
  close() {
    if (this.closed) {
      return;
    }
    this.closed = true;
    this.dispatchEvent(new Event('close'));
  }

  static get last() {
    return FakeWebSocket.all.at(-1);
  }
}

/** Replaces the global `WebSocket` until `vi.unstubAllGlobals()`. */
export const stubWebSocket = () => {
  FakeWebSocket.all = [];
  vi.stubGlobal('WebSocket', FakeWebSocket);
};
