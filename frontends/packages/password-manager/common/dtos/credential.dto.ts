type UUID = string;
type FieldType = "meta" | "sensitive" | "additional" | "totp";
export type Field = {
	fieldId?: string;
	fieldName: string;
	fieldValue: string;
	fieldType: FieldType;
};

export type UsersForDataSync = {
	id: string;
	publicKey: string;
};

export type UserEncryptedFields = {
	userId: string;
	fields: Field[];
};

export type CredentialBasic = {
	credentialId: string;
	credentialFields: Field[];
};

export type Credential = CredentialBasic & {
	name: string;
	accessType: string;
	description: string;
	folderId: UUID;
	domain: string;
	credentialType: string;
	createdAt: string;
	createdBy: string;
	updatedAt: string;
};

export type SearchedCredential = {
	description: string;
	domain: string;
	folderId: UUID;
	folderName: string;
	credentialId: UUID;
	name: string;
};

export type CredentialFields = {
	credentialId: string;
	fields: BasicFields[];
};

export type BasicFields = {
	fieldId: string;
	fieldValue: string;
};

export type UserEncryptedCredentials = {
	userId: string;
	credentials: CredentialFields[];
};

export type AddCredentialField = {
	fieldName: string;
	fieldValue: string;
	sensitive: boolean;
};

export type CredentialFieldWithId = AddCredentialField & {
	id: string;
};

export type InjectionPayload = {
	id: string | undefined;
	username: string;
};

type fieldType = "meta" | "sensitive" | "additional" | "totp";
export type CredentialFieldComponentProps = {
	fieldId?: string;
	fieldName: string;
	fieldValue: string;
	fieldType?: fieldType;
	sensitive: boolean;
};

export type UrlCredMap = {
	value: string;
	credentialId: string;
};

export type EncryptedEditField = {
	fieldValue: string;
	userId: string;
};

export type CapturedCredentialData = {
	username: string;
	password: string;
	domain: string;
	url: string;
};

export type EditedUserField = {
	fieldId: string;
	fieldName: string;
	fieldType: string;
	fieldValues: EncryptedEditField[];
};

export type EnvField = {
	envFieldId: string;
	fieldValue: string;
};

export type NewFieldPayload = {
	fieldName: string;
	fieldType: string;
	fieldValues: {
		fieldValue: string;
		userId: string;
		envFieldValues: { envId: string; fieldValue: string }[];
	}[];
};
