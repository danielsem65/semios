import { mountToolbar } from './toolbar';
import { createOverlayController } from './overlay-controller';

const HOST_ID = 'semios-overlay';
const START_ID = 'start';

function createHost(): HTMLElement {
  const host = document.createElement('div');
  host.id = HOST_ID;
  const style = host.style;
  style.setProperty('all', 'initial', 'important');
  style.setProperty('position', 'fixed', 'important');
  style.setProperty('top', '0', 'important');
  style.setProperty('left', '0', 'important');
  style.setProperty('right', '0', 'important');
  style.setProperty('display', 'block', 'important');
  style.setProperty('z-index', '2147483647', 'important');
  style.setProperty('color-scheme', 'light dark', 'important');
  return host;
}

function attach(): void {
  if (document.getElementById(HOST_ID) || document.getElementById(START_ID)) return;
  const host = createHost();
  (document.body ?? document.documentElement).appendChild(host);
  mountToolbar(host, createOverlayController(), true);
}

export function start(): void {
  if (document.body) {
    attach();
    return;
  }
  const pending = (): void => attach();
  document.addEventListener('DOMContentLoaded', pending, { once: true });
  document.addEventListener('readystatechange', pending, { once: true });
}
