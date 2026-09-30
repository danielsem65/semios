const BRIDGE_ORIGIN = 'semios://';

const send = (action: string, arg?: string): void => {
  const suffix = arg === undefined ? '' : `/${encodeURIComponent(arg)}`;
  window.location.href = `${BRIDGE_ORIGIN}${action}${suffix}`;
};

/** `window.open` is often handed a relative path, which Rust would reject as
 * unparsable, so it is resolved against the page before it leaves. */
function absolute(raw: string): string | null {
  try {
    return new URL(raw, window.location.href).href;
  } catch {
    return null;
  }
}

/**
 * A link that asks for a new window means a new tab, not a second window.
 *
 * The click is caught on `document` in the capture phase, so a page cannot get
 * there first and decide otherwise, and `window.open` is wrapped because a
 * redirect can request a window with no element to click. Returning null is
 * what a blocked popup gets, which is the shape pages already handle.
 *
 * Desktop also intercepts at the wry level. That catches forms and anything the
 * script never sees, but it is not available on Android, so this half is what
 * makes the behaviour the same on both.
 */
export function interceptNewWindows(): void {
  document.addEventListener(
    'click',
    (event) => {
      const target = event.target as Element | null;
      const link = target?.closest?.('a[href]') as HTMLAnchorElement | null;
      if (!link) return;
      if ((link.getAttribute('target') ?? '').toLowerCase() !== '_blank') return;
      const url = absolute(link.href);
      // Never swallow our own bridge traffic, or a menu item would recurse.
      if (!url || url.startsWith(BRIDGE_ORIGIN)) return;
      event.preventDefault();
      event.stopPropagation();
      send('newtab', url);
    },
    true,
  );

  const original = window.open;
  const patched = (url?: unknown, ...rest: unknown[]): unknown => {
    if (url === undefined || url === null) {
      return (original as (...args: unknown[]) => unknown).apply(window, [url, ...rest]);
    }
    const target = absolute(String(url));
    if (target && !target.startsWith(BRIDGE_ORIGIN)) {
      send('newtab', target);
      return null;
    }
    return (original as (...args: unknown[]) => unknown).apply(window, [url, ...rest]);
  };
  window.open = patched as unknown as typeof window.open;
}
