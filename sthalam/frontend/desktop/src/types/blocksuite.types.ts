export interface Block {
  id: string;
  type: "heading" | "text" | "image" | "container" | "html" | "notice-board" | "form";
  x: number;
  y: number;
  width: number;
  height: number;
  zIndex: number;
  content: string;
  styles: Record<string, any>;
}

export interface Viewport {
  x: number;
  y: number;
  zoom: number;
}

export interface UserInfo {
  name: string;
  color: string;
  id: number;
  userId: string;
}

export interface Collaborator {
  id: string;
  name: string;
  color: string;
  clientId: number;
}

export interface UserDetails {
  userId: string;
  deviceId: string;
  username: string;
  publicKey: string;
  deviceKey: string;
}

export interface NoticeMessage {
  id: string;
  username: string;
  message: string;
  timestamp: number;
  userId: string;
}

export interface FormField {
  id: string;
  type: "text" | "email" | "number" | "textarea" | "checkbox";
  label: string;
  placeholder?: string;
  required: boolean;
}

export interface FormConfig {
  fields: FormField[];
  submitButtonText: string;
}
