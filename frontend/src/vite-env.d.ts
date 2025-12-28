/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_API_URL: string
  readonly VITE_TELEGRAM_BOT_USERNAME: string
  readonly VITE_VERSION: string
  // Add more env variables as needed
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}

// Global types for WASM integration
declare global {
  interface Window {
    wasmModule?: any;
    wasmInitialized?: boolean;
  }
}

// WASM types for starscalendars_wasm_astro
declare module '../wasm-astro/starscalendars_wasm_astro.js' {
  export default function init(input?: any): Promise<any>;
  export function compute_state(jd: number): number;
  export function set_state_extended_enabled(enabled: boolean): void;
  export function next_winter_solstice_from(jd: number): number;
  export function get_version(): string;
  export const memory: WebAssembly.Memory;
}
