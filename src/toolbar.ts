import css from './toolbar.css?inline';
import { EMPTY_SNAPSHOT, type Controller, type Snapshot } from './controller';
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
};

const MARKUP = `
<div class="progress"><i></i></div>
<div class="bar">
  <button class="btn" data-act="back" title="Back (Alt+Left)" aria-label="Back">${ICON.back}</button>
  <button class="btn" data-act="forward" title="Forward (Alt+Right)" aria-label="Forward">${ICON.forward}</button>
  <button class="btn" data-act="reload" title="Reload (Ctrl+R)" aria-label="Reload">${ICON.reload}</button>
  <button class="btn" data-act="home" title="Home" aria-label="Home">${ICON.home}</button>
  <form class="field" autocomplete="off">
    <span class="hint">${ICON.lock}</span>
    <input type="text" spellcheck="false" autocapitalize="off" autocorrect="off" placeholder="Search or enter address" aria-label="Address and search bar" />
    <button class="clear" type="button" title="Clear" aria-label="Clear">${ICON.close}</button>
  </form>
  <button class="btn" data-act="close" title="Close window" aria-label="Close window">${ICON.close}</button>
</div>`;

const isDesktop = (platform: string) =>
  platform === 'windows' || platform === 'macos' || platform === 'linux';

export function mountToolbar(
  host: HTMLElement,
  controller: Controller,
  collapsible: boolean,
): void {
  const root = host.shadowRoot ?? host.attachShadow({ mode: 'open' });
  const shell = document.createElement('div');
  shell.innerHTML = MARKUP;
  root.append(shell);
  applyStyles(root, css);

  const bar = pick<HTMLElement>(shell, '.bar');
  const form = pick<HTMLFormElement>(shell, '.field');
  const input = pick<HTMLInputElement>(shell, 'input');
  const hint = pick<HTMLElement>(shell, '.hint');
  const clear = pick<HTMLButtonElement>(shell, '.clear');
  const fill = pick<HTMLElement>(shell, '.progress i');
  const back = pick<HTMLButtonElement>(shell, '[data-act="back"]');
  const forward = pick<HTMLButtonElement>(shell, '[data-act="forward"]');
  const reload = pick<HTMLButtonElement>(shell, '[data-act="reload"]');
  const close = pick<HTMLButtonElement>(shell, '[data-act="close"]');

  let snapshot: Snapshot = EMPTY_SNAPSHOT;
  let editing = false;

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

  bar.addEventListener('click', (event) => {
    const target = (event.target as Element | null)?.closest<HTMLButtonElement>('[data-act]');
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
      case 'close':
        controller.close();
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

      if (mod && key === 'l') {
        event.preventDefault();
        input.focus();
        input.select();
      } else if ((mod && key === 'r') || event.key === 'F5') {
        event.preventDefault();
        controller.reload();
      } else if (mod && key === 'w' && isDesktop(snapshot.platform)) {
        event.preventDefault();
        controller.close();
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

  if (collapsible) {
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

  paint();
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
