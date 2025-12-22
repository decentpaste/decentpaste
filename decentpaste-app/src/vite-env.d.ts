/// <reference types="vite/client" />

declare module '*.svg?url' {
  const url: string;
  export default url;
}

declare module '*.svg' {
  const content: string;
  export default content;
}
