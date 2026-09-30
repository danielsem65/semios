export interface Update {
  available: boolean;
  version: string;
  notes: string;
  url: string;
  asset: string;
  checked: boolean;
}

export interface TabInfo {
  id: number;
  title: string;
  url: string;
}

export interface Snapshot {
  url: string;
  loading: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
  platform: string;
  update: Update;
  tabs: TabInfo[];
  activeTab: number;
}

export interface Controller {
  go(input: string): void;
  back(): void;
  forward(): void;
  reload(): void;
  stop(): void;
  home(): void;
  newTab(input?: string): void;
  selectTab(id: number): void;
  closeTab(id: number): void;
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
  // No state yet. The toolbar hides the list rather than inventing tabs, and
  // Android remote pages stay here permanently: they cannot receive a push.
  tabs: [],
  activeTab: 0,
};
