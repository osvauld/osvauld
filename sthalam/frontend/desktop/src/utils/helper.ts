// Mock function for POC - will be replaced with real Tauri invoke later
export async function getUserDetails() {
  // TODO: Replace with actual Tauri invoke call
  // import { invoke } from "@tauri-apps/api/core";
  // return await invoke('get_user_details');

  // For POC, return mock data
  return {
    userId: "user-123",
    deviceId: "device-456",
    username: "DemoUser", // Change this to test different usernames
    publicKey: "mock-public-key",
    deviceKey: "mock-device-key"
  };
}
