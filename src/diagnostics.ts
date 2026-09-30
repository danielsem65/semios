type TauriInternals = {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
};

function internals(): TauriInternals | undefined {
  return (window as unknown as { __TAURI_INTERNALS__?: TauriInternals }).__TAURI_INTERNALS__;
}

export type Level = 'INFO' | 'WARN' | 'ERROR';

/**
 * Report to the Rust log. Deliberately avoids the bundled Tauri API: the
 * injected script is a classic IIFE that runs before page scripts, and it still
 * has to report when the page itself failed to load.
 */
export function report(level: Level, message: string): void {
  const line = `[semios] ${level}: ${message}`;
  try {
    const invoke = internals()?.invoke;
    if (invoke) {
      void invoke('browser_log', { level, message }).catch(() => console.warn(line));
      return;
    }
  } catch {
    // Fall through to the console.
  }
  console.warn(line);
}

/**
 * Install error listeners as early as possible. Capture is required because
 * resource failures (a module script that never loads, for example) do not
 * bubble, so a bubbling listener would never see the one failure that matters
 * most here.
 */
export function watchRuntimeErrors(): void {
  window.addEventListener(
    'error',
    (event) => {
      const target = event.target as { tagName?: string; src?: string } | null;
      if (target && target !== window && target.tagName) {
        report('ERROR', `resource failed tag=${target.tagName} src=${target.src ?? 'inline'}`);
        return;
      }
      report(
        'ERROR',
        `uncaught: ${event.message} @ ${event.filename}:${event.lineno}:${event.colno}`,
      );
    },
    true,
  );
  window.addEventListener('unhandledrejection', (event) => {
    report('ERROR', `unhandled rejection: ${String(event.reason)}`);
  });
}

export function describePage(stage: string): void {
  const start = document.getElementById('start');
  const bar = document.getElementById('bar');
  report(
    'INFO',
    `${stage} url=${location.href} ready=${document.readyState}` +
      ` start=${start ? 'yes' : 'no'} bar=${bar ? 'yes' : 'no'}` +
      ` scripts=${document.scripts.length}`,
  );
}
