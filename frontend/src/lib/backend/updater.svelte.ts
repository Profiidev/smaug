import { browser } from '$app/environment';
import { invalidate } from '$app/navigation';
import { sleep } from 'positron-components/util/interval.svelte';

export enum UpdateType {
  Nodes = 'Nodes',
  Settings = 'Settings',
  Users = 'Users',
  Groups = 'Groups'
}

export type UpdateMessage = {
  type:
    | UpdateType.Nodes
    | UpdateType.Settings
    | UpdateType.Users
    | UpdateType.Groups;
};

let updater: WebSocket | undefined | false = $state(browser && undefined);
let interval: number;
let disconnect = false;

export const connectWebsocket = () => {
  if (updater === false || updater) return;
  createWebsocket();
};

const createWebsocket = () => {
  updater = new WebSocket('/api/ws/updater');

  updater.onmessage = (event) => {
    const msg: UpdateMessage = JSON.parse(event.data);
    handleMessage(msg);
  };

  updater.onclose = async () => {
    clearInterval(interval);
    if (disconnect) return;
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
      invalidate((url) => url.pathname.startsWith('/api/nodes'));
      break;
    }
    case UpdateType.Settings: {
      invalidate((url) => url.pathname.startsWith('/api/settings'));
      break;
    }
    case UpdateType.Users: {
      invalidate((url) => url.pathname.startsWith('/api/user'));
      break;
    }
    case UpdateType.Groups: {
      invalidate((url) => url.pathname.startsWith('/api/group'));
      break;
    }
  }
};
