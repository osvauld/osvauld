/**
 * WebGPU Device Manager
 * Handles GPU initialization and resource management
 */

export class GPUDeviceManager {
  private static instance: GPUDeviceManager | null = null;
  private device: GPUDevice | null = null;
  private adapter: GPUAdapter | null = null;
  private isInitialized = false;
  private initPromise: Promise<boolean> | null = null;

  private constructor() {}

  static getInstance(): GPUDeviceManager {
    if (!GPUDeviceManager.instance) {
      GPUDeviceManager.instance = new GPUDeviceManager();
    }
    return GPUDeviceManager.instance;
  }

  /**
   * Initialize WebGPU device
   */
  async initialize(): Promise<boolean> {
    // Return cached result if already initialized
    if (this.isInitialized) {
      return true;
    }

    // Return existing promise if initialization in progress
    if (this.initPromise) {
      return this.initPromise;
    }

    this.initPromise = this._doInitialize();
    return this.initPromise;
  }

  private async _doInitialize(): Promise<boolean> {
    try {
      // Check WebGPU support
      if (!navigator.gpu) {
        console.warn('[WebGPU] WebGPU not supported in this browser');
        return false;
      }

      // Request adapter
      this.adapter = await navigator.gpu.requestAdapter({
        powerPreference: 'high-performance',
      });

      if (!this.adapter) {
        console.warn('[WebGPU] Failed to get GPU adapter');
        return false;
      }

      // Request device
      this.device = await this.adapter.requestDevice();

      // Listen for device lost
      this.device.lost.then((info) => {
        console.error('[WebGPU] Device lost:', info.message);
        this.isInitialized = false;
        this.device = null;
      });

      this.isInitialized = true;
      console.log('✅ [WebGPU] Device initialized successfully');
      console.log('[WebGPU] Adapter info:', {
        vendor: this.adapter.info?.vendor,
        architecture: this.adapter.info?.architecture,
        device: this.adapter.info?.device,
      });

      return true;
    } catch (error) {
      console.error('[WebGPU] Initialization failed:', error);
      return false;
    }
  }

  /**
   * Get GPU device (null if not initialized)
   */
  getDevice(): GPUDevice | null {
    return this.device;
  }

  /**
   * Get GPU adapter (null if not initialized)
   */
  getAdapter(): GPUAdapter | null {
    return this.adapter;
  }

  /**
   * Check if WebGPU is available and initialized
   */
  isAvailable(): boolean {
    return this.isInitialized && this.device !== null;
  }

  /**
   * Get adapter limits for optimization decisions
   */
  getAdapterLimits(): GPUSupportedLimits | null {
    return this.adapter?.limits ?? null;
  }
}

// Export singleton instance
export const gpuDevice = GPUDeviceManager.getInstance();
