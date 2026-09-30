export interface Snapshot {
  url: string;
  loading: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
  platform: string;
}

export interface Controller {
  go(input: string): void;
  back(): void;
  forward(): void;
  reload(): void;
  stop(): void;
  home(): void;
  close(): void;
  subscribe(receiver: (snapshot: Snapshot) => void): void;
}

export const EMPTY_SNAPSHOT: Snapshot = {
  url: '',
  loading: false,
  canGoBack: false,
  canGoForward: false,
  platform: '',
};
