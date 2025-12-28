/**
 * TypeScript shim for WASM modules during development
 * This file provides type declarations for dynamic WASM imports
 */

declare module '*/starscalendars_wasm_astro.js' {
  export default function init(): Promise<void>;
  export const memory: WebAssembly.Memory;
  export function compute_state(julianDay: number): number;
  export function set_state_extended_enabled(enabled: boolean): void;
  export function get_version(): string;
  export function get_version(): string;
}

declare module '*/mock_starscalendars_wasm_astro.js' {
  export default function init(): Promise<void>;
  export const memory: WebAssembly.Memory;
  export function compute_state(julianDay: number): number;
  export function set_state_extended_enabled(enabled: boolean): void;
  export function get_version(): string;
  export function get_version(): string;
}
