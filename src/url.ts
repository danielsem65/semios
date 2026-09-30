export const HOME_URL = 'https://duckduckgo.com/';

const SEARCH_ENDPOINT = 'https://duckduckgo.com/?q=';

const SCHEME = /^[a-z][a-z0-9+.-]*:/i;
const HOSTLIKE = /^[^\s/?#]+\.[^\s/?#]{2,}(?:[/:?#]|$)/;
const LOCALHOST = /^(?:localhost|127(?:\.\d{1,3}){3}|\[::1\])(?::\d+)?(?:[/?#]|$)/i;

export function resolveInput(raw: string): string {
  const input = raw.trim();
  if (input.length === 0) return HOME_URL;
  if (SCHEME.test(input)) return input;
  if (input.startsWith('//')) return `https:${input}`;
  if (!/\s/.test(input) && (HOSTLIKE.test(input) || LOCALHOST.test(input))) {
    return `https://${input}`;
  }
  return SEARCH_ENDPOINT + encodeURIComponent(input);
}

export function displayUrl(raw: string): string {
  if (raw.length === 0) return '';
  try {
    const url = new URL(raw);
    if (url.protocol !== 'http:' && url.protocol !== 'https:') return raw;
    const path = url.pathname === '/' ? '' : url.pathname;
    return `${url.host}${path}${url.search}${url.hash}`;
  } catch {
    return raw;
  }
}

export function isWebUrl(raw: string): boolean {
  try {
    const url = new URL(raw);
    return url.protocol === 'http:' || url.protocol === 'https:';
  } catch {
    return false;
  }
}
