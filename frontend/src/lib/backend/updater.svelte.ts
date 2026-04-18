import { browser } from '$app/environment';
import { invalidate } from '$app/navigation';
import { sleep } from '@profidev/pleiades/util/interval.svelte';

export enum UpdateType {
  Nodes = 'Nodes',
  Settings = 'Settings',
  Users = 'Users',
  Groups = 'Groups'
}

export interface UpdateMessage {
  type:
    | UpdateType.Nodes
    | UpdateType.Settings
    | UpdateType.Users
    | UpdateType.Groups;
}

let updater: WebSocket | undefined | false = $state(browser && undefined);
let interval = 0;
let disconnect = false;

export const connectWebsocket = () => {
  if (updater === false || updater) {
    return;
  }
  createWebsocket();
};

const createWebsocket = () => {
  updater = new WebSocket('/api/ws/updater');

  // oxlint-disable-next-line prefer-add-event-listener
  updater.onmessage = (event) => {
    const msg: UpdateMessage = JSON.parse(event.data);
    handleMessage(msg);
  };

  // oxlint-disable-next-line prefer-add-event-listener
  updater.onclose = async () => {
    clearInterval(interval);
    if (disconnect) {
      return;
    }
    await sleep(1000);
    createWebsocket();
  };

  // oxlint-disable-next-line no-unsafe-type-assertion
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
  }, 10_000) as unknown as number;
};

export const disconnectWebsocket = () => {
  if (updater) {
    disconnect = true;
    updater.close();
    updater = undefined;
  }
};

const handleMessage = (msg: UpdateMessage) => {
  switch (msg.type) {
    case UpdateType.Nodes: {
      const _ = invalidate((url) => url.pathname.startsWith('/api/nodes'));
      break;
    }
    case UpdateType.Settings: {
      const _ = invalidate((url) => url.pathname.startsWith('/api/settings'));
      break;
    }
    case UpdateType.Users: {
      const _ = invalidate((url) => url.pathname.startsWith('/api/user'));
      break;
    }
    case UpdateType.Groups: {
      const _ = invalidate((url) => url.pathname.startsWith('/api/group'));
      break;
    }
    default: {
      break;
    }
  }
};
