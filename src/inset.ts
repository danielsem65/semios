const STYLE_ID = 'semios-inset';

// The toolbar is fixed to the top of the viewport, so on a real site it sits on
// top of the document rather than pushing it down. Everything in that strip
// belongs to the bar, not the page, which is why links and buttons underneath
// it stopped responding. Reserving the bar's height on the document is what
// makes the page reachable again.
const SHEET = `
:root { --semios-inset: 0px; }
html { padding-top: var(--semios-inset) !important; }
`;

/**
 * Keeps the document inset by the height of the bar, and releases it when the
 * bar hides on scroll.
 *
 * Changing the padding moves the whole page under the viewport, so scroll
 * position is corrected alongside it. Without that correction, removing the
 * padding shifts the page up far enough to look like an upward scroll, which
 * the toolbar reads as "show me again", and the bar flickers forever.
 */
export function reserveBarSpace(host: HTMLElement): void {
  if (document.getElementById(STYLE_ID)) return;

  const style = document.createElement('style');
  style.id = STYLE_ID;
  style.textContent = SHEET;
  (document.head ?? document.documentElement).append(style);

  let applied = -1;

  const apply = (next: number): void => {
    if (next === applied) return;
    const delta = next - applied;
    applied = next;
    document.documentElement.style.setProperty('--semios-inset', `${next}px`);

    const y = window.scrollY;
    if (delta !== 0 && y > 0) {
      window.scrollTo(0, Math.max(0, y - delta));
    }
  };

  const height = (): number => {
    if (host.hasAttribute('hidden-bar')) return 0;
    return Math.ceil(host.getBoundingClientRect().height);
  };

  const measure = (): void => {
    apply(height());
  };

  measure();
  new MutationObserver(measure).observe(host, {
    attributes: true,
    attributeFilter: ['hidden-bar'],
  });
  window.addEventListener('resize', measure, { passive: true });
}
