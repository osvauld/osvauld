type AccessType = "reader" | "manager";
type AccessSource = "acquired" | "inherited";
type UserType = "admin" | "superadmin" | "user";

export type User = {
	id: string;
	name: string;
	username: string;
	publicKey?: string;
	type?: UserType;
	status?: string;
	checked?: boolean;
	accessType?: AccessType;
	accessSource?: AccessSource;
};

type GroupAccessType = "admin" | "member";
export type GroupUser = {
	id: string;
	name: string;
	username: string;
	publicKey: string;
	accessType: GroupAccessType;
};
