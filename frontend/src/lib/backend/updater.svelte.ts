import { browser } from '$app/environment';
import { invalidate } from '$app/navigation';
import { sleep } from 'positron-components/util/interval.svelte';

export enum UpdateType {
  Nodes = 'Nodes'
}

export type UpdateMessage = {
  type: UpdateType.Nodes;
};

let updater: WebSocket | undefined | false = $state(browser && undefined);
let interval: number;

export const connectWebsocket = () => {
  if (updater === false || updater) return;
  createWebsocket();
};

const createWebsocket = () => {
  updater = new WebSocket('/api/ws/updater');

  updater.onerror = (event) => {
    console.log('WebSocket error:', event);
  };

  updater.onmessage = (event) => {
    const msg: UpdateMessage = JSON.parse(event.data);
    handleMessage(msg);
  };

  updater.onclose = async () => {
    clearInterval(interval);
    await sleep(1000);
    createWebsocket();
  };

  interval = setInterval(() => {
    if (
      !updater ||
      updater.readyState === updater.CLOSING ||
      updater.readyState === updater.CLOSED
    ) {
      clearInterval(interval);
      return;
    }

    updater.send('heartbeat');
  }, 10000) as unknown as number;
};

const handleMessage = (msg: UpdateMessage) => {
  switch (msg.type) {
    case UpdateType.Nodes: {
      invalidate((url) => url.pathname.startsWith('/api/nodes'));
      break;
    }
  }
};
