import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Controller, Snapshot } from './controller';
import { resolveInput } from './url';

export function createIpcController(): Controller {
  const call = (action: string, arg?: string): void => {
    void invoke('browser_command', { action, arg: arg ?? null });
  };

  return {
    go(input: string): void {
      call('go', resolveInput(input));
    },
    back(): void {
      call('back');
    },
    forward(): void {
      call('forward');
    },
    reload(): void {
      call('reload');
    },
    stop(): void {
      call('stop');
    },
    home(): void {
      call('home');
    },
    close(): void {
      call('close');
    },
    subscribe(receive: (snapshot: Snapshot) => void): void {
      // Android cannot receive pushed state: Tauri delivers events to a webview
      // by evaluating a dispatch script, and that eval aborts the process. Poll
      // there instead so the toolbar still tracks the current URL and progress.
      if (isMobile()) {
        const tick = (): void => {
          void invoke<Snapshot>('browser_state')
            .then(receive)
            .catch(() => undefined);
        };
        tick();
        window.setInterval(tick, 400);
        return;
      }
      void listen<Snapshot>('semios-state', (event) => receive(event.payload));
      void invoke<Snapshot>('browser_state').then(receive);
    },
  };
}

function isMobile(): boolean {
  return /android|iphone|ipad|ipod/i.test(navigator.userAgent);
}
