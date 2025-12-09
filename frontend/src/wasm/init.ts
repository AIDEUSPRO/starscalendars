/**
 * WASM Astronomical Module Integration (TypeScript 5.9.2 + Babylon.js 8 major — latest minor at runtime)
 *
 * High-performance WASM integration with zero-copy data transfer and strict typing.
 * Implements exactly one compute_state() call per frame for O(1) performance.
 */

import type {
  PositionBuffer,
  WASMError,
} from './types.js';
import { WASMErrorType, validateWASMBuffer } from './types.js';

// Prod flag for conditional logging (Vite-only, no Node typings needed)
const __IS_PROD__ = (() => {
  try {
    // Vite exposes a boolean PROD flag and MODE string
    const env = (typeof import.meta !== 'undefined' && (import.meta as any).env) || undefined;
    return Boolean(env?.PROD || env?.MODE === 'production');
  } catch {
    return false;
  }
})();

// ✅ CORRECT - Result type pattern for strict error handling (TypeScript 5.9.2+)
export type Result<T, E> = { success: true; data: T } | { success: false; error: E };

// ✅ CORRECT - Strict interface definitions for astronomical data (FIXED: Cartesian from WASM)
export interface CelestialPosition {
  readonly x: number;          // Cartesian X coordinate (AU)
  readonly y: number;          // Cartesian Y coordinate (AU)
  readonly z: number;          // Cartesian Z coordinate (AU)
  readonly distance: number;   // Distance from origin (AU)
  readonly timestamp: number;  // Milliseconds since epoch
}

export interface AstronomicalState {
  readonly sun: CelestialPosition;
  readonly earth: CelestialPosition;
  readonly moon: CelestialPosition;
  readonly earthSunDistance: number; // Earth distance from Sun in AU
  readonly sunZenithLat: number;     // Latitude where Sun is in zenith (degrees N-positive)
  readonly sunZenithLng: number;     // Longitude where Sun is in zenith (degrees E-positive)
  readonly sunZenithLatRad: number;  // Latitude in radians
  readonly sunZenithLngRad: number;  // Longitude in radians (E-positive)
}

// ✅ CORRECT - WASM module interface with strict typing for spiritual astronomy
export interface WASMModule {
  readonly compute_state: (julianDay: number) => number;
  readonly get_version: () => string;
  readonly memory: WebAssembly.Memory;
  readonly next_winter_solstice_from: (julianDayUtcStart: number) => number;
  readonly get_mean_obliquity: (julianDay: number) => number;
  readonly get_apparent_sidereal_time: (julianDay: number) => number;
  // NT (Quantum Time)
  readonly get_quantum_time_components: (epochMs: number, timezoneOffsetMinutes: number) => number;
  readonly earth_perihelion_aphelion_for_year_utc?: (yearUtc: number) => number; // ptr to 6 f64
  // Optional: timescale overrides for leap seconds (forward-compatible)
  readonly set_tai_minus_utc_override?: (seconds: number) => void;
  readonly clear_tai_minus_utc_override?: () => void;
}

// ✅ CORRECT - Performance monitoring for WASM operations
class WASMPerformanceTimer {
  private readonly operationName: string;
  private readonly startTime: number;

  constructor(operationName: string) {
    this.operationName = operationName;
    this.startTime = performance.now();
    if (!__IS_PROD__) console.log(`🚀 WASM: Starting ${operationName}`);
  }

  public mark(checkpoint: string): void {
    const currentTime = performance.now();
    const duration = currentTime - this.startTime;
    if (!__IS_PROD__) console.log(`📊 WASM: ${this.operationName} - ${checkpoint} at ${duration.toFixed(3)}ms`);
  }
}

// ✅ CORRECT - Global WASM module state
let globalWasmModule: WASMModule | null = null;
let isInitialized = false;
let initializationPromise: Promise<WASMModule> | null = null;

/**
 * Initialize WASM module with runtime module loading
 */
export const initializeWASM = async (): Promise<WASMModule> => {
  // Singleton pattern to prevent multiple initializations
  if (isInitialized && globalWasmModule) {
    return globalWasmModule;
  }

  if (initializationPromise) {
    return initializationPromise;
  }

  initializationPromise = (async (): Promise<WASMModule> => {
    const timer = new WASMPerformanceTimer('wasm_module_initialization');

    try {
      // Use wasm-pack bundler wrapper (correct for wasm-bindgen glue)
      const wrapper = await import('../wasm-astro/starscalendars_wasm_astro.js');
      // After wrapper loads, the module is initialized and __wbindgen_start() executed
      // Import the wasm module namespace to access memory export
      const wasmNs = (await import('../wasm-astro/starscalendars_wasm_astro_bg.wasm')) as unknown as { memory?: WebAssembly.Memory };

      const compute_state_raw = (wrapper as unknown as { compute_state?: (jd: number) => number }).compute_state;
      const next_winter_solstice_from_raw = (wrapper as unknown as { next_winter_solstice_from?: (jd: number) => number }).next_winter_solstice_from;
      const get_version_fn = (wrapper as unknown as { get_version?: () => string }).get_version;
      const get_mean_obliquity_raw = (wrapper as unknown as { get_mean_obliquity?: (jd: number) => number }).get_mean_obliquity;
      const get_apparent_sidereal_time_raw = (wrapper as unknown as { get_apparent_sidereal_time?: (jd: number) => number }).get_apparent_sidereal_time;
      const get_quantum_time_components_raw = (wrapper as unknown as { get_quantum_time_components?: (ms: number, tzmin: number) => number }).get_quantum_time_components;
      const set_tai_minus_utc_override_raw = (wrapper as unknown as { set_tai_minus_utc_override?: (s: number) => void }).set_tai_minus_utc_override;
      const clear_tai_minus_utc_override_raw = (wrapper as unknown as { clear_tai_minus_utc_override?: () => void }).clear_tai_minus_utc_override;
      const earth_perihelion_aphelion_for_year_utc_raw = (wrapper as unknown as { earth_perihelion_aphelion_for_year_utc?: (y: number) => number }).earth_perihelion_aphelion_for_year_utc;
      const memory = wasmNs.memory;

      if (!memory) throw new Error('WASM memory export missing');
      if (typeof compute_state_raw !== 'function') throw new Error('WASM export compute_state missing');
      if (typeof get_version_fn !== 'function') throw new Error('WASM get_version export missing');
      if (typeof next_winter_solstice_from_raw !== 'function') throw new Error('WASM export next_winter_solstice_from missing');
      if (typeof get_mean_obliquity_raw !== 'function') throw new Error('WASM export get_mean_obliquity missing');
      if (typeof get_apparent_sidereal_time_raw !== 'function') throw new Error('WASM export get_apparent_sidereal_time missing');
      if (typeof get_quantum_time_components_raw !== 'function') throw new Error('WASM export get_quantum_time_components missing');

      // Smoke test
            const testJD = 2451545.0; // J2000 epoch
      const testResult = compute_state_raw(testJD);
      if (testResult === 0) throw new Error('WASM compute_state returned null pointer during init test');

      timer.mark('wasm_functions_loaded');

      // Version via wrapper (wasm-bindgen JS glue handles decoding)
      const version = get_version_fn();
      if (typeof version !== 'string' || version.length === 0) {
        throw new Error('WASM get_version returned invalid value');
      }

      timer.mark('validation_complete');

      // Create WASM module interface (minimal surface used by the scene)
      const module: WASMModule = {
        compute_state: (julianDay: number): number => {
          if (!Number.isFinite(julianDay)) throw new Error(`Invalid Julian Day: ${julianDay}`);
          return compute_state_raw(julianDay);
        },
        get_version: (): string => version,
        memory,
        next_winter_solstice_from: (jdStartUtc: number) => {
          if (!Number.isFinite(jdStartUtc)) throw new Error(`Invalid Julian Day: ${jdStartUtc}`);
          return next_winter_solstice_from_raw(jdStartUtc);
        },
        get_mean_obliquity: (jd: number) => {
          if (!Number.isFinite(jd)) throw new Error(`Invalid Julian Day: ${jd}`);
          return get_mean_obliquity_raw(jd);
        },
        get_apparent_sidereal_time: (jd: number) => {
          if (!Number.isFinite(jd)) throw new Error(`Invalid Julian Day: ${jd}`);
          return get_apparent_sidereal_time_raw(jd);
        },
        get_quantum_time_components: (ms: number, tzmin: number) => {
          if (!Number.isFinite(ms) || !Number.isFinite(tzmin)) throw new Error('Invalid inputs to get_quantum_time_components');
          return get_quantum_time_components_raw(ms, tzmin);
        },
      };

      // Attach optional functions only if present to satisfy exactOptionalPropertyTypes
      if (typeof set_tai_minus_utc_override_raw === 'function') {
        (module as any).set_tai_minus_utc_override = (s: number) => set_tai_minus_utc_override_raw(s);
      }
      if (typeof clear_tai_minus_utc_override_raw === 'function') {
        (module as any).clear_tai_minus_utc_override = () => clear_tai_minus_utc_override_raw();
      }
      if (typeof earth_perihelion_aphelion_for_year_utc_raw === 'function') {
        (module as any).earth_perihelion_aphelion_for_year_utc = (y: number) => earth_perihelion_aphelion_for_year_utc_raw(y);
      }

      globalWasmModule = module;
      isInitialized = true;
      timer.mark('module_ready');

      if (!__IS_PROD__) {
        console.log(`✅ WASM Module Initialized Successfully: Version: ${version}; Memory: ${memory.buffer.byteLength} bytes`);
      }

      return module;

    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown WASM initialization error';
      console.error(`❌ WASM Initialization Failed: ${errorMessage}`);

      // Reset state on failure
      isInitialized = false;
      globalWasmModule = null;
      initializationPromise = null;

      throw new Error(`WASM module initialization failed: ${errorMessage}`);
    }
  })();

  return initializationPromise;
};

/**
 * Get current WASM module instance (if initialized)
 */
export const getWASMModule = (): WASMModule | null => {
  return globalWasmModule;
};

/**
 * Check if WASM module is initialized and ready
 */
export const isWASMInitialized = (): boolean => {
  return isInitialized && globalWasmModule !== null;
};

/**
 * Convert JavaScript Date to Julian Day for WASM calculations
 */
export const dateToJulianDay = (date: Date): number => {
  const unixEpochJulianDay = 2440587.5;
  const millisecondsPerDay = 86400000;
  return unixEpochJulianDay + date.getTime() / millisecondsPerDay;
};

/**
 * Convert milliseconds since epoch to Julian Day
 */
export const millisecondsToJulianDay = (milliseconds: number): number => {
  const unixEpochJulianDay = 2440587.5;
  const millisecondsPerDay = 86400000;
  return unixEpochJulianDay + milliseconds / millisecondsPerDay;
};

/**
 * ✅ CRITICAL: Create zero-copy Float64Array view of WASM memory (TypeScript 5.9.2+)
 *
 * This function implements the zero-copy data transfer requirement with strict validation.
 * Returns a validated Float64Array view directly into WASM memory at the given pointer.
 */
export const createPositionsView = (wasmModule: WASMModule, ptr: number): Result<PositionBuffer, WASMError> => {
  if (ptr === 0) {
    return {
      success: false,
      error: {
        type: WASMErrorType.MemoryAccessError,
        message: 'Null pointer returned from WASM compute_state',
        timestamp: performance.now()
      }
    };
  }

  // Expect 11 f64: Sun(3) + Moon(3) + Earth(3) + Zenith(2)
  const coordinateCount = 11;

  // No mock data allowed - WASM must be available for real astronomical calculations

  // Real WASM memory access with bounds checking
  try {
    const memoryBuffer = wasmModule.memory.buffer;

    // Check if pointer is within valid memory bounds
    if (ptr < 0 || ptr + (coordinateCount * 8) > memoryBuffer.byteLength) {
      return {
        success: false,
        error: {
          type: WASMErrorType.MemoryAccessError,
          message: `Pointer ${ptr} out of bounds (memory size: ${memoryBuffer.byteLength})`,
          timestamp: performance.now()
        }
      };
    }

    const buffer = new Float64Array(memoryBuffer, ptr, coordinateCount);

    // Add debug logging for first few calls
    if (!((globalThis as any).__bufferDebugCount)) (globalThis as any).__bufferDebugCount = 0;
    if ((globalThis as any).__bufferDebugCount++ < 3) {
      console.log(`🔍 WASM Buffer #${(globalThis as any).__bufferDebugCount}:`, {
        ptr,
        coordinateCount,
        bufferLength: buffer.length,
        firstFewValues: Array.from(buffer.slice(0, 9)).map(v => v.toFixed(6))
      });
    }

    const validation = validateWASMBuffer(buffer);
    if (!validation.success) {
      return {
        success: false,
        error: validation.error
      };
    }

    return {
      success: true,
      data: validation.data
    };
  } catch (error) {
    return {
      success: false,
      error: {
        type: WASMErrorType.MemoryAccessError,
        message: `Memory access failed: ${error instanceof Error ? error.message : 'Unknown error'}`,
        timestamp: performance.now()
      }
    };
  }
};

/**
 * Create zero-copy Float64Array view for SEM (Sun/Earth/Moon) buffer
 */
// SEM-only view removed. Unified STATE is used instead.

/**
 * Create zero-copy Float64Array view for STATE buffer (S/E/M + zenith rad)
 * Layout length: 11 (9 coords + 2 zenith values)
 */
// createSTATEView no longer needed; scene inlines zero-copy view

/**
  * ✅ CRITICAL: STATE buffer layout (compute_state)
  * - 0..2: Sun xyz (geocentric; AU)
  * - 3..5: Moon xyz (geocentric; AU)
  * - 6..8: Earth xyz (heliocentric; AU)
  * - 9:    Solar zenith longitude (radians, east-positive)
  * - 10:   Solar zenith latitude (radians)
 */
// Old extractCelestialPositions removed; state provides zenith inline

// Removed JS fallback zenith calculators: we rely exclusively on WASM

/**
 * Cleanup WASM module resources
 */
export const cleanupWASM = (): void => {
  console.log('🧹 Cleaning up WASM module resources');
  globalWasmModule = null;
  isInitialized = false;
  initializationPromise = null;
};
