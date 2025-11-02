export interface Block {
  id: string;
  type: string; // Changed from union to string for flexibility
  x: number;
  y: number;
  width: number;
  height: number;
  zIndex: number;
  content: string;
  styles: Record<string, any>;

  // Optional properties for various block types
  name?: string;
  css?: string;
  visible?: boolean | string; // Can be boolean or CEL expression

  // Tree hierarchy properties
  parentId?: string;
  isEntryPoint?: boolean;
  isModal?: boolean;

  // Navigation and form properties
  action?: string;
  targetContainerId?: string;
  targetScreenId?: string;
  formId?: string;
  fieldName?: string;
  value?: any;

  // Manual connections for flow diagrams
  manualConnections?: Array<{
    id: string;
    target: string;
    color?: string;
    arrowSize?: number;
    style?: string;
  }>;

  // Branching logic
  branchingQuestionId?: string;
  branchValue?: any;
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
