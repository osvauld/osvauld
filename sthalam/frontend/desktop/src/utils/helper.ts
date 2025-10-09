import { invoke } from "@tauri-apps/api/core";

/**
 * Send message to Tauri backend
 * Follows livnote's sendMessage pattern
 */
export async function sendMessage(command: string, payload?: any): Promise<any> {
  try {
    const result = await invoke(command, payload ? { payload } : {});
    return result;
  } catch (error) {
    console.error(`Error invoking ${command}:`, error);
    throw error;
  }
}

/**
 * Get user details from backend
 */
export async function getUserDetails() {
  // For POC, return mock data until backend is ready
  // TODO: Replace with: return await sendMessage('getUserDetails');

  // Create a proper base64 encoded deviceId for clientId generation
  // This encodes a 32-byte random string
  const mockDeviceBytes = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    mockDeviceBytes[i] = Math.floor(Math.random() * 256);
  }
  const mockDeviceId = btoa(String.fromCharCode(...mockDeviceBytes));

  return {
    userId: "user-123",
    deviceId: mockDeviceId,
    username: "DemoUser",
    publicKey: "mock-public-key",
    deviceKey: "mock-device-key"
  };
}
