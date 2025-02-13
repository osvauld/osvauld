import type {
	SafariCredential,
	FirefoxCredential,
	ChromeCredential,
	LastpassCredential,
	BitwardenCredential,
	ProtonCredential,
	DashlaneCredential,
	NordpassCredential,
	KeepassCredential,
	RoboformCredential,
	OnepasswordCredential,
	CredentialImportType,
	IntermediateCredential,
	CredentialData,
} from "../dtos/import.dto";


import { addCredentialHandler } from "./addCredentialHelper";

const extractUsername = (username: string, email: string): string => {
	return username || email;
};

const extractTOTPSecret = (uri: string): string | null => {
	const match = uri.match(/[?&]secret=([^&]+)/);
	return match ? match[1] : null;
};

export const isSafariCredential = (
	credential: CredentialImportType,
): credential is SafariCredential => {
	return "Title" in credential && "URL" in credential;
};

export const isFirefoxCredential = (
	credential: CredentialImportType,
): credential is FirefoxCredential => {
	return "url" in credential && "guid" in credential;
};

// Chrome, edge CSVs and Opera CSVs follow similar format

export const isDashlaneCredential = (
	credential: CredentialImportType,
): credential is DashlaneCredential => {
	return (
		"username" in credential && "url" in credential && "note" in credential
	);
};

export const isKeepassCredential = (
	credential: CredentialImportType,
): credential is KeepassCredential => {
	return (
		"Web Site" in credential &&
		"Login Name" in credential &&
		"Comments" in credential
	);
};

export const isRoboformCredential = (
	credential: CredentialImportType,
): credential is RoboformCredential => {
	return (
		"MatchUrl" in credential && "Pwd" in credential && "Login" in credential
	);
};

export const isNordpassCredential = (
	credential: CredentialImportType,
): credential is NordpassCredential => {
	return "name" in credential && "url" in credential && "note" in credential;
};

export const is1passwordCredential = (
	credential: CredentialImportType,
): credential is OnepasswordCredential => {
	return (
		"Title" in credential && "Url" in credential && "OTPAuth" in credential
	);
};

export const isChromeCredential = (
	credential: CredentialImportType,
): credential is ChromeCredential => {
	return "name" in credential && "url" in credential && "note" in credential;
};

export const isLastpassCredential = (
	credential: CredentialImportType,
): credential is LastpassCredential => {
	return "name" in credential && "url" in credential;
};

export const isBitwardenCredential = (
	credential: CredentialImportType,
): credential is BitwardenCredential => {
	return "login_username" in credential && "login_uri" in credential;
};

export const isProtonCredential = (
	credential: CredentialImportType,
): credential is ProtonCredential => {
	return "username" in credential && "email" in credential;
};

export const transformSafariCredentials = (parsedData: CredentialImportType[]) => {
	return parsedData.filter(isSafariCredential).map((credential) => ({
		name: credential.Title,
		description: credential.Notes,
		domain: credential.URL,
		username: credential.Username,
		password: credential.Password,
	}));
};

export const transformFirefoxCredentials = (parsedData: CredentialImportType[]) => {
	return parsedData.filter(isFirefoxCredential).map((credential) => ({
		name: `Login - ${new URL(credential.url).hostname}`,
		description: `Created on ${new Date(+credential.timeCreated)}`,
		domain: credential.url,
		username: credential.username,
		password: credential.password,
	}));
};

export const transformChromeCredentials = (parsedData: CredentialImportType[]) => {
	return parsedData.filter(isChromeCredential).map((credential) => ({
		name: credential.name,
		description: credential.note,
		domain: credential.url,
		username: credential.username,
		password: credential.password,
	}));
};

export const transformLastpassCredentials = (parsedData: CredentialImportType[]) => {
	return parsedData
		.filter(isLastpassCredential)
		.filter((credential) => credential.username && credential.password)
		.map((credential) => ({
			name: credential.name,
			description: credential.extra,
			domain: credential.url,
			username: credential.username,
			password: credential.password,
			totp: credential.totp,
		}));
};

export const transformBitwardenCredentials = (parsedData: CredentialImportType[]) => {
	return parsedData
		.filter(isBitwardenCredential)
		.filter((credential) => credential.type === "login")
		.map((credential) => ({
			name: credential.name,
			description: credential.notes,
			domain: credential.login_uri,
			username: credential.login_username,
			password: credential.login_password,
			totp: credential.login_totp,
		}));
};

export const transformProtonpassCredentials = (parsedData: CredentialImportType[]) => {
	return parsedData
		.filter(isProtonCredential)
		.filter((credential) => credential.type === "login")
		.map((credential) => {
			const extractedUsername = extractUsername(
				credential.username,
				credential.email,
			);

			const result: IntermediateCredential = {
				name: credential.name,
				description: credential.note,
				domain: credential.url,
				username: extractedUsername,
				password: credential.password,
			};

			if (extractedUsername === credential.username && credential.email) {
				result.email = credential.email;
			}

			if (credential.totp) {
				const parsedTotp = extractTOTPSecret(credential.totp);
				if (parsedTotp) {
					result.totp = parsedTotp;
				}
			}

			return result;
		});
};

export const transformDashlaneCredentials = (parsedData: CredentialImportType[]) => {
	return parsedData.filter(isDashlaneCredential).map((credential) => ({
		name: credential.title,
		description: credential.note,
		domain: credential.url,
		username: credential.username,
		password: credential.password,
		totp: credential.otpSecret,
	}));
};

export const transformNordpassCredentials = (parsedData: CredentialImportType[]) => {
	return parsedData
		.filter(isNordpassCredential)
		.filter((credential) => credential.type === "password")
		.map((credential) => ({
			name: credential.name,
			description: credential.note,
			domain: credential.url,
			username: credential.username,
			password: credential.password,
		}));
};

export const transformKeepassCredentials = (parsedData: CredentialImportType[]) => {
	return parsedData.filter(isKeepassCredential).map((credential) => ({
		name: `Login - ${new URL(credential["Web Site"]).hostname}`,
		description: credential.Comments,
		domain: credential["Web Site"],
		username: credential["Login Name"],
		password: credential.Password,
	}));
};

export const transformRoboformCredentials = (parsedData: CredentialImportType[]) => {
	return parsedData.filter(isRoboformCredential).map((credential) => ({
		name: credential.Name,
		description: credential.Note,
		domain: credential.Url,
		username: credential.Login,
		password: credential.Pwd,
	}));
};

export const transformOnepasswordCredentials = (parsedData: CredentialImportType[]) => {
	return parsedData.filter(is1passwordCredential).map((credential) => ({
		name: credential.Title,
		description: credential.Notes,
		domain: credential.Url,
		username: credential.Username,
		password: credential.Password,
		totp: credential.OTPAuth,
	}));
};

export const finalProcessing = async (
	folderId: string,
	credentialData: CredentialData,
) => {
	try {
		const fieldPayload: {
			fieldName: string;
			fieldValue: string;
			fieldType: string;
		}[] = [];

		let addCredentialPayload: {
			name: string;
			description: string;
			credentialType: string;
			credentialFields: {
				fieldName: string;
				fieldValue: string;
				fieldType: string;
			}[];
		} = {
			name: "",
			description: "",
			credentialType: "Login",
			credentialFields: [],
		};

		if (credentialData.username) {
			fieldPayload.push({
				fieldName: "Username",
				fieldValue: credentialData.username,
				fieldType: "meta",
			});
		}

		if (credentialData.password) {
			fieldPayload.push({
				fieldName: "Password",
				fieldValue: credentialData.password,
				fieldType: "sensitive",
			});
		}

		if (credentialData.domain) {
			fieldPayload.push(
				{
					fieldName: "Domain",
					fieldValue: credentialData.domain,
					fieldType: "additional",
				},
				{
					fieldName: "URL",
					fieldValue: credentialData.domain,
					fieldType: "meta",
				},
			);
		}

		if (credentialData.totp) {
			fieldPayload.push({
				fieldName: "TOTP",
				fieldValue: credentialData.totp,
				fieldType: "totp",
			});
		}
		if (credentialData.email) {
			fieldPayload.push({
				fieldName: "Email",
				fieldValue: credentialData.email,
				fieldType: "sensitive",
			});
		}

		addCredentialPayload.credentialFields = fieldPayload;

		if (credentialData.name) {
			addCredentialPayload.name = credentialData.name;
		}

		if (credentialData.description) {
			addCredentialPayload.description = credentialData.description;
		}

		const response = await addCredentialHandler(addCredentialPayload, folderId);

		return { success: response.success };
	} catch (error) {
		console.error("Error posting credential:", error);
		return { success: false, error };
	}
};
