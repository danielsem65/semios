import { mountToolbar } from './toolbar';
import { createOverlayController } from './overlay-controller';
import { describePage, report, watchRuntimeErrors } from './diagnostics';
import { interceptNewWindows } from './newwindow';
import { reserveBarSpace } from './inset';

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
  if (document.getElementById(HOST_ID)) {
    report('WARN', 'overlay attach skipped: host already present');
    return;
  }
  if (document.getElementById(START_ID)) {
    report('INFO', 'overlay attach skipped: start page owns its own toolbar');
    return;
  }
  const host = createHost();
  (document.body ?? document.documentElement).append(host);
  try {
    // Installed before the toolbar so a link is never clickable in the gap.
    interceptNewWindows();
    const controller = createOverlayController();
    mountToolbar(host, controller, {
      collapsible: true,
      menu: {
        openInTab: (url) => controller.newTab(url),
        reload: () => controller.reload(),
      },
    });
    // The start page owns its own layout and reserves room for its bar, but a
    // site we do not control knows nothing about ours, so we inset it for them.
    reserveBarSpace(host);
    report('INFO', `overlay attached url=${location.href}`);
  } catch (error) {
    report('ERROR', `overlay mount failed: ${String(error)}`);
    host.remove();
  }
}

export function start(): void {
  watchRuntimeErrors();
  describePage('overlay init');
  if (document.body) {
    attach();
    return;
  }
  const pending = (): void => attach();
  document.addEventListener('DOMContentLoaded', pending, { once: true });
  document.addEventListener('readystatechange', pending, { once: true });
}
