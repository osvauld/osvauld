/**
 * Core types for Sthalam application
 */

export interface Website {
  id: string;
  name: string;
  description?: string;
  default?: boolean;
}

/**
 * BlockSuite content structure stored in backend
 * Simplified - state_vector not needed for basic storage
 */
export interface BlocksuiteContent {
  main_doc: number[];  // Yjs encoded document updates
  client_id: string;
  last_modified: number;
  title: string;
}

export interface Resource {
  id: string;
  title: string;
  resourceType: 'website';  // For now, only website type
  websiteId: string;        // The website/folder this resource belongs to
  lastModified: number;
  favourite?: boolean;
  preview?: string;
  content?: BlocksuiteContent; // Full BlockSuite content when loaded
}

export interface UserDetails {
  userId: string;
  deviceId: string;
  username: string;
  publicKey: string;
  deviceKey: string;
}
