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

const totpUrlValidator = (credentialFields: any) => {
   // This function loops checks for few conditions inside credentials fields

    // Checks if TOTP field is inside credentialFields
	let totpPresence = credentialFields.find(
		(field: CredentialFieldComponentProps) => {
			if (field.fieldName === "TOTP" && field.fieldValue.length !== 0) {
				return field;
			}
		},
	);
    
	// Incase TOTP found, checking validity of it
	if (totpPresence) {
		const isTotpValid = totpValidator(totpPresence.fieldValue);
		if (!isTotpValid) {
			return {
				success: false,
				message: "TOTP entered is Invalid!",
			};
		}
	}

     // Checks if URL field is inside credentialFields
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

    // Incase URL found, checking validity of it
	if (!isValidUrl) {
		return {
			success: false,
			message: "Invalid URL",
		};
	}

	// If no issues found, returning success message
	return {
		success: true,
		message: "Successful Validation",
	};

	// If any issue found, operation is aborted at this stage
};

export const addCredentialHandler = async (
	credentialData: any,
	folderId: string,
) => {
	const { credentialFields, name, description, credentialType } =
		credentialData;

	const fieldValidationResponse: { success: boolean; message: string } = totpUrlValidator(credentialFields);

	if (!fieldValidationResponse.success) return fieldValidationResponse;
    
	// proceeding if nothing wrong found during validation
	let addCredentialFields: Field[] = [];

	for (const field of credentialFields) {
		// If field is URL some transformations are done and field data is pushed to temp state (addCredentialFields)
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

		// for all others except Domain,  some transformations are done and field data is pushed to temp state (addCredentialFields)
		if (
			field.fieldName.length !== 0 && field.fieldValue.length !== 0 &&
			field.fieldName !== "Domain"
		) {
			const baseField: Field = {
				fieldName: field.fieldName,
				fieldValue: field.fieldValue,
				fieldType: field.sensitive ? "sensitive" : "meta",
			};

			//if field totp, small change need to be done
			if (field.fieldName === "TOTP") {
				if (field.fieldValue.length !== 0) {
					baseField.fieldType = "totp";
					addCredentialFields.push(baseField);
				} else {
                    // This else blck checks if infact in totp field.fieldValue.length === 0, if so pushing to base fields aborted
					continue;
				}
			} 
		
			addCredentialFields.push(baseField);
			
		}
	}

	if(addCredentialFields.length === 0 ) return  {
		success: false,
		message: "Please add fields",
	};

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
