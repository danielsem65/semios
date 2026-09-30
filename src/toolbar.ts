import css from './toolbar.css?inline';
import { EMPTY_SNAPSHOT, EMPTY_UPDATE, type Controller, type Snapshot, type TabInfo } from './controller';
import { mountContextMenu, type ContextMenuActions } from './menu';
import { displayUrl, isWebUrl } from './url';

const ICON = {
  back: '<svg viewBox="0 0 24 24"><path d="M15 5l-7 7 7 7"/></svg>',
  forward: '<svg viewBox="0 0 24 24"><path d="M9 5l7 7-7 7"/></svg>',
  reload: '<svg viewBox="0 0 24 24"><path d="M20 12a8 8 0 1 1-2.7-6"/><path d="M20 4v5h-5"/></svg>',
  stop: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="6.2"/></svg>',
  home: '<svg viewBox="0 0 24 24"><path d="M4 11l8-7 8 7"/><path d="M6.5 9.8V20h11V9.8"/></svg>',
  lock: '<svg viewBox="0 0 24 24"><rect x="5" y="11" width="14" height="9" rx="2.2"/><path d="M8.2 11V8a3.8 3.8 0 0 1 7.6 0v3"/></svg>',
  globe: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8"/><path d="M4 12h16"/><path d="M12 4c2.8 3 2.8 13 0 16-2.8-3-2.8-13 0-16z"/></svg>',
  close: '<svg viewBox="0 0 24 24"><path d="M6.5 6.5l11 11M17.5 6.5l-11 11"/></svg>',
  update: '<svg viewBox="0 0 24 24"><path d="M12 4.5v10"/><path d="M7.8 10.8 12 15l4.2-4.2"/><path d="M5 19.5h14"/></svg>',
  add: '<svg viewBox="0 0 24 24"><path d="M12 5.5v13M5.5 12h13"/></svg>',
};

const MARKUP = `
<div class="chrome">
  <div class="progress"><i></i></div>
  <div class="tabs">
    <div class="tablist" role="tablist"></div>
    <button class="btn add" data-act="newtab" title="New tab (Ctrl+T)" aria-label="New tab">${ICON.add}</button>
  </div>
  <div class="bar">
    <button class="btn" data-act="back" title="Back (Alt+Left)" aria-label="Back">${ICON.back}</button>
    <button class="btn" data-act="forward" title="Forward (Alt+Right)" aria-label="Forward">${ICON.forward}</button>
    <button class="btn" data-act="reload" title="Reload (Ctrl+R)" aria-label="Reload">${ICON.reload}</button>
    <button class="btn" data-act="home" title="Home" aria-label="Home">${ICON.home}</button>
    <button class="btn update" data-act="update" title="Update available" aria-label="Update available" hidden>${ICON.update}</button>
    <form class="field" autocomplete="off">
      <span class="hint">${ICON.lock}</span>
      <input type="text" spellcheck="false" autocapitalize="off" autocorrect="off" placeholder="Search or enter address" aria-label="Address and search bar" />
      <button class="clear" type="button" title="Clear" aria-label="Clear">${ICON.close}</button>
    </form>
    <button class="btn" data-act="close" title="Close window" aria-label="Close window">${ICON.close}</button>
  </div>
</div>`;

export interface ToolbarOptions {
  /** Hides the chrome on scroll down, and only on pages we do not own. */
  collapsible: boolean;
  menu?: ContextMenuActions;
}

const isDesktop = (platform: string) =>
  platform === 'windows' || platform === 'macos' || platform === 'linux';

export function mountToolbar(
  host: HTMLElement,
  controller: Controller,
  options: ToolbarOptions,
): ShadowRoot {
  const root = host.shadowRoot ?? host.attachShadow({ mode: 'open' });
  const shell = document.createElement('div');
  shell.innerHTML = MARKUP;
  root.append(shell);
  applyStyles(root, css);

  const list = pick<HTMLElement>(shell, '.tablist');
  const form = pick<HTMLFormElement>(shell, '.field');
  const input = pick<HTMLInputElement>(shell, 'input');
  const hint = pick<HTMLElement>(shell, '.hint');
  const clear = pick<HTMLButtonElement>(shell, '.clear');
  const fill = pick<HTMLElement>(shell, '.progress i');
  const back = pick<HTMLButtonElement>(shell, '[data-act="back"]');
  const forward = pick<HTMLButtonElement>(shell, '[data-act="forward"]');
  const reload = pick<HTMLButtonElement>(shell, '[data-act="reload"]');
  const update = pick<HTMLButtonElement>(shell, '[data-act="update"]');
  const close = pick<HTMLButtonElement>(shell, '[data-act="close"]');

  let snapshot: Snapshot = EMPTY_SNAPSHOT;
  let editing = false;

  const makeTab = (tab: TabInfo): HTMLElement => {
    const node = document.createElement('div');
    node.className = 'tab';
    node.dataset.act = 'selecttab';
    node.dataset.id = String(tab.id);
    node.setAttribute('role', 'tab');
    const selected = tab.id === snapshot.activeTab;
    node.setAttribute('aria-selected', selected ? 'true' : 'false');
    node.title = tab.url || tab.title;
    const label = document.createElement('span');
    label.className = 'tab-label';
    label.textContent = tab.title || 'New tab';
    const x = document.createElement('button');
    x.type = 'button';
    x.className = 'tab-close';
    x.dataset.act = 'closetab';
    x.dataset.id = String(tab.id);
    x.title = 'Close tab';
    x.setAttribute('aria-label', `Close ${tab.title || 'tab'}`);
    x.innerHTML = ICON.close;
    node.append(label, x);
    return node;
  };

  const paint = (): void => {
    back.toggleAttribute('disabled', !snapshot.canGoBack);
    forward.toggleAttribute('disabled', !snapshot.canGoForward);
    close.hidden = !isDesktop(snapshot.platform);
    reload.innerHTML = snapshot.loading ? ICON.stop : ICON.reload;
    reload.dataset.act = snapshot.loading ? 'stop' : 'reload';
    reload.title = snapshot.loading ? 'Stop (Esc)' : 'Reload (Ctrl+R)';
    hint.innerHTML = isWebUrl(snapshot.url) ? ICON.lock : ICON.globe;
    if (!editing) input.value = displayUrl(snapshot.url);
    form.classList.toggle('dirty', editing && input.value.length > 0);
    // Rebuilt rather than diffed. The list is a handful of short strings, and a
    // reconcile path here would be more code than the repaint it replaced.
    list.replaceChildren(...(snapshot.tabs ?? []).map(makeTab));
    // The button is only there when a newer release actually exists for this
    // platform, so a failed or skipped check leaves the bar exactly as it was.
    const next = snapshot.update ?? EMPTY_UPDATE;
    update.hidden = !next.available;
    if (next.available) update.title = `Update to ${next.version}`;
  };

  const progress = (loading: boolean): void => {
    if (loading) {
      fill.style.opacity = '1';
      fill.style.width = '18%';
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          fill.style.width = '82%';
        });
      });
      return;
    }
    fill.style.width = '100%';
    setTimeout(() => {
      fill.style.opacity = '0';
      fill.style.width = '0%';
    }, 260);
  };

  shell.addEventListener('click', (event) => {
    const target = (event.target as Element | null)?.closest<HTMLElement>('[data-act]');
    const id = Number(target?.dataset.id);
    switch (target?.dataset.act) {
      case 'back':
        controller.back();
        break;
      case 'forward':
        controller.forward();
        break;
      case 'home':
        controller.home();
        break;
      case 'reload':
        controller.reload();
        break;
      case 'stop':
        controller.stop();
        break;
      case 'newtab':
        controller.newTab();
        break;
      case 'selecttab':
        // A tab body is not a button, so there is no id when the click missed
        // one; treat that as a no-op rather than selecting tab NaN.
        if (Number.isFinite(id)) controller.selectTab(id);
        break;
      case 'closetab':
        if (Number.isFinite(id)) controller.closeTab(id);
        break;
      case 'close':
        controller.close();
        break;
      case 'update':
        controller.installUpdate();
        break;
      default:
        break;
    }
  });

  input.addEventListener('focus', () => {
    editing = true;
    input.select();
    paint();
  });

  input.addEventListener('input', () => {
    form.classList.toggle('dirty', input.value.length > 0);
  });

  input.addEventListener('blur', () => {
    editing = false;
    paint();
  });

  input.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    event.stopPropagation();
    input.blur();
  });

  form.addEventListener('submit', (event) => {
    event.preventDefault();
    controller.go(input.value);
    input.blur();
  });

  clear.addEventListener('click', () => {
    input.value = '';
    input.focus();
  });

  window.addEventListener(
    'keydown',
    (event) => {
      if (event.defaultPrevented) return;
      const mod = event.ctrlKey || event.metaKey;
      const key = event.key.toLowerCase();
      const open = (snapshot.tabs ?? []).length;

      if (mod && key === 'l') {
        event.preventDefault();
        input.focus();
        input.select();
      } else if ((mod && key === 'r') || event.key === 'F5') {
        event.preventDefault();
        controller.reload();
      } else if (mod && key === 't') {
        event.preventDefault();
        controller.newTab();
      } else if (mod && key === 'w') {
        // With tabs open, Ctrl+W closes the tab. With only one, it closes the
        // window, because on desktop that is what the user is asking for.
        if (open > 1) {
          event.preventDefault();
          controller.closeTab(snapshot.activeTab);
        } else if (isDesktop(snapshot.platform)) {
          event.preventDefault();
          controller.close();
        }
      } else if (event.altKey && event.key === 'ArrowLeft' && snapshot.canGoBack) {
        event.preventDefault();
        controller.back();
      } else if (event.altKey && event.key === 'ArrowRight' && snapshot.canGoForward) {
        event.preventDefault();
        controller.forward();
      } else if (event.key === 'Escape' && snapshot.loading) {
        event.preventDefault();
        controller.stop();
      }
    },
    true,
  );

  if (options.collapsible) {
    let last = window.scrollY;
    window.addEventListener(
      'scroll',
      () => {
        const now = window.scrollY;
        const delta = now - last;
        if (now < 64) host.removeAttribute('hidden-bar');
        else if (delta > 6) host.setAttribute('hidden-bar', '');
        else if (delta < -6) host.removeAttribute('hidden-bar');
        if (Math.abs(delta) > 6) last = now;
      },
      { passive: true },
    );
  }

  controller.subscribe((next) => {
    const wasLoading = snapshot.loading;
    snapshot = next;
    paint();
    if (wasLoading !== next.loading) progress(next.loading);
  });

  if (options.menu) mountContextMenu(root, options.menu);
  paint();
  return root;
}

function pick<T extends Element>(scope: ParentNode, selector: string): T {
  const found = scope.querySelector<T>(selector);
  if (!found) throw new Error(`semios: missing element ${selector}`);
  return found;
}

function applyStyles(root: ShadowRoot, cssText: string): void {
  try {
    const sheet = new CSSStyleSheet();
    sheet.replaceSync(cssText);
    root.adoptedStyleSheets = [sheet];
  } catch {
    const style = document.createElement('style');
    style.textContent = cssText;
    root.append(style);
  }
}
