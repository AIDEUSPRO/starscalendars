import * as React from 'react';
import { useCallback, useEffect, useMemo, useState } from 'react';
// import UIOverlay from './components/UIOverlay';
import BabylonScene from './scene/BabylonScene';
import './styles/BabylonScene.css';
import { initializeWASM, type AstronomicalState, type WASMModule } from './wasm/init';

// ✅ CORRECT - Result type pattern (TypeScript 5.9.2+ strict compliance)
type Result<T, E> = { success: true; data: T } | { success: false; error: E };

// ✅ CORRECT - Strict interface definitions with readonly properties for immutability
interface AppState {
  readonly wasmModule: WASMModule | null;
  readonly isInitialized: boolean;
  readonly currentJulianDay: number;
  readonly astronomicalData: AstronomicalState | null;
  readonly error: string | null;
  readonly frameCount: number;
  readonly currentFPS: number;
}

// ✅ CORRECT - Performance timer class for development monitoring
class PerformanceTimer {
  private readonly operationName: string;
  private readonly startTime: number;

  constructor(operationName: string) {
    this.operationName = operationName;
    this.startTime = performance.now();
    // Only log in development and not for frame updates
    if (import.meta.env.DEV && operationName !== 'frame_update') {
      console.log(`🚀 Frontend: Starting ${operationName}`);
    }
  }

  public mark(checkpoint: string): void {
    // Only log in development and not for frame updates
    if (import.meta.env.DEV && this.operationName !== 'frame_update') {
      const currentTime = performance.now();
      const duration = currentTime - this.startTime;
      console.log(`📊 Frontend: ${this.operationName} - ${checkpoint} at ${duration.toFixed(3)}ms`);
    }
  }
}

// ✅ CORRECT - Constants for zero-allocation reference
const JULIAN_DAY_UNIX_EPOCH = 2440587.5; // Julian Day for Unix epoch

const App: React.FC = () => {
  // ✅ CORRECT - Strict state typing with pre-allocated initial state
  const [appState, setAppState] = useState<AppState>({
    wasmModule: null,
    isInitialized: false,
    currentJulianDay: JULIAN_DAY_UNIX_EPOCH + Date.now() / 86400000.0,
    astronomicalData: null,
    error: null,
    frameCount: 0,
    currentFPS: 0
  });

  // Canvas и UI управляются BabylonScene

  // ✅ CORRECT - Memoized error boundary component (zero re-allocation)
  const ErrorBoundary = useMemo(() => ({ children, error }: { children: React.ReactNode; error: string | null }) => {
    if (error) {
      return (
        <div className="error-container" role="alert">
          <h2>WASM Integration Error</h2>
          <pre className="error-details">{error}</pre>
          <button
            type="button"
            onClick={() => window.location.reload()}
            className="error-retry-button"
          >
            Reload Application
          </button>
        </div>
      );
    }
    return <>{children}</>;
  }, []);

  // ✅ CORRECT - WASM initialization with proper error handling (async/await pattern)
  const initializeAstronomy = useCallback(async (): Promise<Result<WASMModule, string>> => {
    const timer = new PerformanceTimer('wasm_initialization');

    try {
      const wasmModule = await initializeWASM();
      timer.mark('wasm_loaded');

      // Жёсткая проверка версии обёртки (без ENV, строго из обёртки)
      const EXPECTED_WASM_WRAPPER_VERSION = '2.0.1';
      try {
        const version = wasmModule.get_version();
        if (version !== EXPECTED_WASM_WRAPPER_VERSION) {
          return {
            success: false,
            error: `WASM wrapper version mismatch: expected '${EXPECTED_WASM_WRAPPER_VERSION}', got '${version}'`
          };
        }
        // eslint-disable-next-line no-console
        console.log(`✅ WASM wrapper version OK: ${version}`);
      } catch {
        return { success: false, error: 'WASM get_version() export not available' };
      }

      timer.mark('validation_complete');
      return { success: true, data: wasmModule };

    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown WASM initialization error';
      return { success: false, error: `WASM initialization failed: ${errorMessage}` };
    }
  }, []);

  // ✅ REMOVED - All astronomical data updates happen in BabylonScene render loop

  // ✅ REMOVED TIMER - All updates happen in Babylon.js render loop at 60fps
  // No separate timers needed - BabylonScene handles all WASM calls

  // ✅ REPLACED with callback ref above - no useEffect needed

  // ✅ CORRECT - Component initialization effect with cleanup
  useEffect(() => {
    let isMounted = true;

    const initializeComponent = async (): Promise<void> => {
      const initTimer = new PerformanceTimer('app_component_init');

      const wasmResult = await initializeAstronomy();

      if (!isMounted) return; // Prevent state update if component unmounted

      if (wasmResult.success) {
        setAppState(prevState => ({
          ...prevState,
          wasmModule: wasmResult.data,
          isInitialized: true,
          error: null
        }));

        // ✅ NO TIMERS - BabylonScene handles all updates at 60fps
        initTimer.mark('initialization_complete');
      } else {
        setAppState(prevState => ({
          ...prevState,
          error: wasmResult.error,
          isInitialized: false
        }));
      }
    };

    initializeComponent().catch((error: unknown) => {
      if (isMounted) {
        const errorMessage = error instanceof Error ? error.message : 'Unexpected initialization error';
        setAppState(prevState => ({
          ...prevState,
          error: errorMessage,
          isInitialized: false
        }));
      }
    });

    // Cleanup function
    return () => {
      isMounted = false;
      // No timers to cleanup - all handled by Babylon.js engine
    };
  }, [initializeAstronomy]);

  // ✅ CORRECT - Loading state with accessibility
  if (!appState.isInitialized && !appState.error) {
    return (
      <div className="app-loading-screen" role="status" aria-live="polite">
        <div className="loading-spinner"></div>
        <p>Initializing WASM astronomical core...</p>
        <small>Frame count: {appState.frameCount}</small>
      </div>
    );
  }

  return (
    <ErrorBoundary error={appState.error}>
      <div className="app-container" style={{ position: 'fixed', inset: 0 }}>
        {/* Minimal overlay only for FPS via Babylon Tools */}
        <div id="stats" className="stats-overlay" />

        {/* Main 3D scene container - Babylon.js integration point */}
        <main className="scene-container">
          {/* Babylon.js 3D Scene Manager - self-managed canvas */}
          <BabylonScene
            wasmModule={appState.wasmModule}
            isInitialized={appState.isInitialized}
          />

          {/* No HTML overlays except stats above. */}
        </main>

      </div>
    </ErrorBoundary>
  );
};

export default App;
