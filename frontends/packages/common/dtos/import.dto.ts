export type SafariCredential = {
	Title: string;
	URL: string;
	Username: string;
	Password: string;
	Notes: string;
	OTPAuth: string;
};

export type FirefoxCredential = {
	url: string;
	username: string;
	password: string;
	httpRealm: string;
	formActionOrigin: string;
	guid: string;
	timeCreated: string;
	timeLastUsed: string;
	timePasswordChanged: string;
};

export type ChromeCredential = {
	name: string;
	url: string;
	username: string;
	password: string;
	note: string;
};

export type LastpassCredential = {
	name: string;
	url: string;
	username: string;
	password: string;
	totp: string;
	extra: string;
};

export type BitwardenCredential = {
	type: string;
	name: string;
	login_uri: string;
	login_username: string;
	login_password: string;
	login_totp: string;
	notes: string;
};

export type ProtonCredential = {
	type: string;
	name: string;
	url: string;
	email: string;
	username: string;
	password: string;
	note: string;
	totp: string;
};

export type DashlaneCredential = {
	username: string;
	title: string;
	password: string;
	note: string;
	url: string;
	otpSecret: string;
};

export type NordpassCredential = {
	name: string;
	url: string;
	username: string;
	password: string;
	note: string;
	type: string;
};

export type OnepasswordCredential = {
	Title: string;
	Url: string;
	Username: string;
	Password: string;
	OTPAuth: string;
	Notes: string;
};

export type RoboformCredential = {
	Name: string;
	Url: string;
	Login: string;
	Pwd: string;
	Note: string;
};

export type KeepassCredential = {
	"Login Name": string;
	Password: string;
	"Web Site": string;
	Comments: string;
};

export type Platform =
	| "Safari"
	| "Firefox"
	| "Chrome"
	| "Opera"
	| "Bitwarden"
	| "Edge"
	| "Dashlane"
	| "1password"
	| "Nordpass"
	| "Keepass"
	| "Protonpass"
	| "Lastpass"
	| "Roboform";

export type CredentialImportType =
	| SafariCredential
	| FirefoxCredential
	| ChromeCredential
	| LastpassCredential
	| BitwardenCredential
	| DashlaneCredential
	| NordpassCredential
	| KeepassCredential
	| RoboformCredential
	| OnepasswordCredential;

export type IntermediateCredential = {
	name: string;
	description: string;
	domain: string;
	username: string;
	password: string;
	totp?: string;
	email?: string;
};

export type CredentialData = {
	username?: string;
	password?: string;
	domain?: string;
	description?: string;
	name?: string;
	totp?: string;
	email?: string;
};

export type ApprovedCredentialSubmitParams = {
	folderId: string;
} & {
	[key: string]: CredentialData;
};
