declare module '*.css?inline' {
  const css: string;
  export default css;
}

interface Window {
  __semios?: {
    update(snapshot: unknown): void;
  };
}
