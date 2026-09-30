import { mountToolbar } from './toolbar';
import { createIpcController } from './ipc-controller';
import { resolveInput } from './url';
import { describePage, report, watchRuntimeErrors } from './diagnostics';
import './start.css';

const SHORTCUTS: ReadonlyArray<readonly [string, string]> = [
  ['GitHub', 'github.com'],
  ['YouTube', 'youtube.com'],
  ['Wikipedia', 'wikipedia.org'],
  ['Hacker News', 'news.ycombinator.com'],
  ['Reddit', 'reddit.com'],
  ['Docs', 'developer.mozilla.org'],
];

function buildStartPage(): void {
  const start = document.querySelector('#start');
  if (!start) return;
  const grid = document.createElement('div');
  grid.className = 'tiles';
  for (const [label, target] of SHORTCUTS) {
    const tile = document.createElement('button');
    tile.className = 'tile';
    tile.type = 'button';
    tile.textContent = label;
    tile.addEventListener('click', () => window.location.assign(resolveInput(target)));
    grid.append(tile);
  }
  start.append(grid);
  report('INFO', `start page built ${SHORTCUTS.length} tiles`);
}

/**
 * A silent bail-out is what made the blank window undiagnosable. Say so on the
 * page itself, not only in the log.
 */
function showFailure(detail: string): void {
  const banner = document.createElement('div');
  banner.setAttribute('role', 'alert');
  banner.style.cssText = [
    'position:fixed',
    'left:0',
    'right:0',
    'bottom:0',
    'z-index:2147483647',
    'margin:0',
    'padding:10px 14px',
    'background:#7f1d1d',
    'color:#fff',
    'font:13px/1.45 system-ui,-apple-system,Segoe UI,sans-serif',
    'white-space:pre-wrap',
    'word-break:break-word',
  ].join(';');
  banner.textContent = `Semios could not finish starting: ${detail}`;
  document.body?.append(banner);
}

function boot(): void {
  describePage('start boot');
  const host = document.querySelector<HTMLElement>('#bar');
  if (!host) {
    report('ERROR', 'start boot aborted: no #bar host element');
    showFailure('missing #bar element');
    return;
  }
  try {
    buildStartPage();
    mountToolbar(host, createIpcController(), false);
    report('INFO', 'start toolbar mounted');
  } catch (error) {
    report('ERROR', `start boot failed: ${String(error)}`);
    showFailure(String(error));
  }
}

watchRuntimeErrors();

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', boot, { once: true });
} else {
  boot();
}
