import { sendMessage, getDomain } from "../utils/helper";
import {
	type CredentialFieldComponentProps,
	type Field,
} from "../dtos/index";

const totpValidator = (secretKey: string): boolean => {
	// Base32 encoded TOTP secrets typically range in size, allowing for flexibility
	if (secretKey.length < 10 || secretKey.length > 64) {
		return false;
	}

	// Check character set: Only uppercase letters A-Z and digits 2-7 are valid
	const validBase32Regex = /^[A-Z2-7]+$/;
	if (!validBase32Regex.test(secretKey)) {
		return false;
	}

	return true;
};

const fieldValidator = (credentialFields: any) => {
	let totpPresence = credentialFields.find(
		(field: CredentialFieldComponentProps) => {
			if (field.fieldName === "TOTP" && field.fieldValue.length !== 0) {
				return field;
			}
		},
	);

	let isValidUrl = !credentialFields.some(
		(field: CredentialFieldComponentProps) => {
			if (field.fieldName === "URL") {
				try {
					getDomain(field.fieldValue);
					return false;
				} catch (_) {
					return true;
				}
			}
			return false;
		},
	);

	if (totpPresence) {
		const isTotpValid = totpValidator(totpPresence.fieldValue);
		if (!isTotpValid) {
			return {
				success: false,
				message: "TOTP entered is Invalid!",
			};
		}
	}
	if (!isValidUrl) {
		return {
			success: false,
			message: "Invalid URL",
		};
	}

	return {
		success: true,
		message: "Successful Validation",
	};
};

export const addCredentialHandler = async (
	credentialData: any,
	folderId: string,
) => {
	const { credentialFields, name, description, credentialType } =
		credentialData;

	const fieldValidationResponse: { success: boolean; message: string } =
		fieldValidator(credentialFields);

	if (!fieldValidationResponse.success) return fieldValidationResponse;

	let addCredentialFields: Field[] = [];

	for (const field of credentialFields) {
		if (field.fieldName === "URL" && field.fieldValue.length !== 0) {
			try {
				if (
					!field.fieldValue.startsWith("https://") &&
					!field.fieldValue.startsWith("http://")
				) {
					return {
						success: false,
						message: "Invalid URL",
					};
				}
				const domain = getDomain(field.fieldValue);
				addCredentialFields.push({
					fieldName: "Domain",
					fieldValue: domain,
					fieldType: "additional",
				});
			} catch (error) {
				return {
					success: false,
					message: "Invalid URL",
				};
			}
		}
		if (
			(field.fieldName.length !== 0 || field.fieldValue.length !== 0) &&
			field.fieldName !== "Domain"
		) {
			const baseField: Field = {
				fieldName: field.fieldName,
				fieldValue: field.fieldValue,
				fieldType: field.sensitive ? "sensitive" : "meta",
			};
			if (field.fieldName === "TOTP") {
				if (field.fieldValue.length !== 0) {
					baseField.fieldType = "totp";
					addCredentialFields.push(baseField);
				} else {
					continue;
				}
			} else {
				addCredentialFields.push(baseField);
			}
		}
	}

	const credentialPayload = JSON.stringify({
		name: name,
		description,
		credentialType,
		credentialFields: addCredentialFields,
	});

	const response = await sendMessage("addCredential", {
		credentialPayload,
		folderId: folderId,
		credentialType: credentialType,
	});

	//  need to return response instead of hard coding
	return {
		success: true,
		message: "Credential added successfully",
	};
};
