type FolderType = "private" | "shared";
type FolderAccessType = "reader" | "manager" | "none";
export type Folder = {
	id: string;
	name: string;
	description?: string;
	accessType?: FolderAccessType;
	type?: FolderType;
	shared?: boolean
};
