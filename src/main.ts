import { mountToolbar } from './toolbar';
import { createIpcController } from './ipc-controller';
import { resolveInput } from './url';
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
}

function boot(): void {
  const host = document.querySelector<HTMLElement>('#bar');
  if (!host) return;
  buildStartPage();
  mountToolbar(host, createIpcController(), false);
}

boot();
