import { EMPTY_SNAPSHOT, type Controller, type Snapshot } from './controller';
import { HOME_URL, resolveInput } from './url';

const BRIDGE_ORIGIN = 'semios://';

const bridge = (action: string, arg?: string): void => {
  const suffix = arg === undefined ? '' : `/${encodeURIComponent(arg)}`;
  window.location.href = `${BRIDGE_ORIGIN}${action}${suffix}`;
};

export function createOverlayController(): Controller {
  let pushed: Snapshot = EMPTY_SNAPSHOT;
  const receivers = new Set<(snapshot: Snapshot) => void>();

  const publish = (loading: boolean): void => {
    pushed = { ...pushed, url: window.location.href, loading };
    for (const receive of receivers) receive(pushed);
  };

  const settle = (): void => publish(false);

  window.addEventListener('load', settle, { once: true });
  window.addEventListener('pageshow', settle, { once: true });
  window.addEventListener('pagehide', () => publish(false), { once: true });

  return {
    go(input: string): void {
      publish(true);
      window.location.href = resolveInput(input);
    },
    back(): void {
      if (pushed.canGoBack) window.history.back();
    },
    forward(): void {
      if (pushed.canGoForward) window.history.forward();
    },
    reload(): void {
      publish(true);
      window.location.reload();
    },
    stop(): void {
      window.stop();
      settle();
    },
    home(): void {
      publish(true);
      window.location.href = HOME_URL;
    },
    close(): void {
      bridge('close');
    },
    subscribe(receive: (snapshot: Snapshot) => void): void {
      receivers.add(receive);
      receive({ ...pushed, url: window.location.href });
      window.__semios = {
        update(snapshot: unknown): void {
          pushed = { ...EMPTY_SNAPSHOT, ...(snapshot as Snapshot) };
          receive({ ...pushed, url: window.location.href });
        },
      };
    },
  };
}
