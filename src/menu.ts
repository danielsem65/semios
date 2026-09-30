import { displayUrl } from './url';

export interface ContextMenuActions {
  openInTab(url: string): void;
  reload(): void;
}

const MENU = 'menu';
const SOURCE = 'source';

let root: ShadowRoot | null = null;
let actions: ContextMenuActions | null = null;
let menu: HTMLElement | null = null;
let panel: HTMLElement | null = null;

/** Copy is not a given: `navigator.clipboard` is missing on a plain http origin
 * and can reject on one that is not focused, so the old selection trick stays as
 * the fallback rather than as a relic. */
async function copy(text: string): Promise<void> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }
  } catch {
    // Fall through to the textarea below.
  }
  const area = document.createElement('textarea');
  area.value = text;
  area.setAttribute('aria-hidden', 'true');
  area.style.cssText = 'position:fixed;left:-9999px;top:0;opacity:0';
  document.body?.append(area);
  area.select();
  try {
    document.execCommand('copy');
  } catch {
    // Nothing left to try; the caller still gets to see the value below.
  }
  area.remove();
}

function close(): void {
  menu?.remove();
  menu = null;
  panel?.remove();
  panel = null;
}

/** `fetch` on the page's own URL is subject to its CSP, and a page that blocks
 * it leaves us with the rendered markup. That is still a page source, and it is
 * better than a menu item that silently does nothing. */
async function showSource(body: HTMLElement, marker: HTMLElement): Promise<void> {
  body.textContent = 'Loading source…';
  let text = '';
  try {
    const response = await fetch(window.location.href, {
      credentials: 'include',
      cache: 'no-store',
    });
    text = await response.text();
  } catch {
    text = `<!doctype html>\n${document.documentElement.outerHTML}`;
  }
  if (panel === marker) body.textContent = text;
}

function openMenu(x: number, y: number, url: string): void {
  if (!root || !actions) return;
  close();

  const node = document.createElement('div');
  node.className = MENU;
  node.setAttribute('role', 'menu');

  const items: Array<[string, () => void]> = [
    ['Open in new tab', () => {
      close();
      actions?.openInTab(url);
    }],
    ['Copy link address', () => {
      close();
      // Copied outside the page, so the user can paste it anywhere, not just in
      // another Semios tab.
      void copy(url);
    }],
    ['View page source', () => {
      close();
      openSource();
    }],
    ['Reload', () => {
      close();
      actions?.reload();
    }],
  ];

  for (const [label, run] of items) {
    const item = document.createElement('button');
    item.type = 'button';
    item.className = `${MENU}-item`;
    item.setAttribute('role', 'menuitem');
    // textContent, never innerHTML: the label is ours but this path also runs
    // on pages we do not control, and there is no reason to parse anything.
    item.textContent = label;
    item.addEventListener('click', (event) => {
      event.preventDefault();
      event.stopPropagation();
      run();
    });
    node.append(item);
  }

  root.append(node);
  const box = node.getBoundingClientRect();
  node.style.left = `${Math.max(8, Math.min(x, window.innerWidth - box.width - 8))}px`;
  node.style.top = `${Math.max(8, Math.min(y, window.innerHeight - box.height - 8))}px`;
  menu = node;
}

function openSource(): void {
  if (!root) return;
  close();

  const node = document.createElement('div');
  node.className = SOURCE;
  node.setAttribute('role', 'dialog');
  node.setAttribute('aria-label', 'Page source');

  const head = document.createElement('div');
  head.className = `${SOURCE}-head`;
  const label = document.createElement('span');
  label.className = `${SOURCE}-label`;
  label.textContent = displayUrl(window.location.href) || window.location.href;
  const done = document.createElement('button');
  done.type = 'button';
  done.className = `${SOURCE}-close`;
  done.textContent = 'Close';
  done.addEventListener('click', (event) => {
    event.preventDefault();
    event.stopPropagation();
    close();
  });
  head.append(label, done);

  const body = document.createElement('pre');
  body.className = `${SOURCE}-body`;

  node.append(head, body);
  root.append(node);
  panel = node;
  void showSource(body, node);
}

/**
 * Right-click handling for a page we do not own.
 *
 * The listener is on `document` in the capture phase, so a page that wants to
 * suppress its own menu cannot, which is the behaviour a browser gives you.
 * Everything renders inside the toolbar's shadow root, so the page's own CSS
 * cannot move it and cannot reach into it.
 */
export function mountContextMenu(shadow: ShadowRoot, handlers: ContextMenuActions): void {
  root = shadow;
  actions = handlers;

  document.addEventListener(
    'contextmenu',
    (event) => {
      // A right click inside our own menu should not relocate it.
      if (event.composedPath().includes(shadow.host)) {
        if (menu) event.preventDefault();
        return;
      }
      const target = event.target as Element | null;
      const link = target?.closest?.('a[href]') as HTMLAnchorElement | null;
      event.preventDefault();
      // With no link under the cursor the source and copy entries still make
      // sense for the page itself, so the same menu opens on the current URL.
      openMenu(event.clientX, event.clientY, link?.href ?? window.location.href);
    },
    true,
  );

  document.addEventListener(
    'pointerdown',
    (event) => {
      if (menu && event.composedPath().includes(shadow.host)) return;
      if (panel && event.composedPath().includes(shadow.host)) return;
      close();
    },
    true,
  );

  document.addEventListener(
    'keydown',
    (event) => {
      if (event.key !== 'Escape') return;
      if (!menu && !panel) return;
      event.preventDefault();
      event.stopPropagation();
      close();
    },
    true,
  );

  // Scrolling a menu out from under the cursor, or a new document arriving
  // under it, both leave it stranded.
  window.addEventListener('scroll', close, { passive: true, capture: true });
  window.addEventListener('pagehide', close);
}
