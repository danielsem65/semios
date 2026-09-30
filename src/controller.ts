export interface Update {
  available: boolean;
  version: string;
  notes: string;
  url: string;
  asset: string;
  checked: boolean;
}

export interface Snapshot {
  url: string;
  loading: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
  platform: string;
  update: Update;
}

export interface Controller {
  go(input: string): void;
  back(): void;
  forward(): void;
  reload(): void;
  stop(): void;
  home(): void;
  close(): void;
  installUpdate(): void;
  subscribe(receiver: (snapshot: Snapshot) => void): void;
}

export const EMPTY_UPDATE: Update = {
  available: false,
  version: '',
  notes: '',
  url: '',
  asset: '',
  checked: false,
};

export const EMPTY_SNAPSHOT: Snapshot = {
  url: '',
  loading: false,
  canGoBack: false,
  canGoForward: false,
  platform: '',
  update: EMPTY_UPDATE,
};
