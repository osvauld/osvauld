// import { sendMessage, getDomain } from "../utils/helper";
// import {
// 	type CredentialFieldComponentProps,
// 	type Field,
// } from "../dtos/index";

// const totpValidator = (secretKey: string): boolean => {
// 	// Base32 encoded TOTP secrets typically range in size, allowing for flexibility
// 	if (secretKey.length < 10 || secretKey.length > 64) {
// 		return false;
// 	}

// 	// Check character set: Only uppercase letters A-Z and digits 2-7 are valid
// 	const validBase32Regex = /^[A-Z2-7]+$/;
// 	if (!validBase32Regex.test(secretKey)) {
// 		return false;
// 	}

// 	return true;
// };

// const totpUrlValidator = (credentialFields: any) => {
//    // This function loops checks for few conditions inside credentials fields

//     // Checks if TOTP field is inside credentialFields
// 	let totpPresence = credentialFields.find(
// 		(field: CredentialFieldComponentProps) => {
// 			if (field.fieldName === "TOTP" && field.fieldValue.length !== 0) {
// 				return field;
// 			}
// 		},
// 	);
    
// 	// Incase TOTP found, checking validity of it
// 	if (totpPresence) {
// 		const isTotpValid = totpValidator(totpPresence.fieldValue);
// 		if (!isTotpValid) {
// 			return {
// 				success: false,
// 				message: "TOTP entered is Invalid!",
// 			};
// 		}
// 	}

//      // Checks if URL field is inside credentialFields
// 	let isValidUrl = !credentialFields.some(
// 		(field: CredentialFieldComponentProps) => {
// 			if (field.fieldName === "URL") {
// 				try {
// 					getDomain(field.fieldValue);
// 					return false;
// 				} catch (_) {
// 					return true;
// 				}
// 			}
// 			return false;
// 		},
// 	);

//     // Incase URL found, checking validity of it
// 	if (!isValidUrl) {
// 		return {
// 			success: false,
// 			message: "Invalid URL",
// 		};
// 	}

// 	// If no issues found, returning success message
// 	return {
// 		success: true,
// 		message: "Successful Validation",
// 	};

// 	// If any issue found, operation is aborted at this stage
// };

// export const addCredentialHandler = async (
// 	credentialData: any,
// 	folderId: string,
// ) => {
// 	const { credentialFields, name, description, credentialType } =
// 		credentialData;

// 	const fieldValidationResponse: { success: boolean; message: string } = totpUrlValidator(credentialFields);

// 	if (!fieldValidationResponse.success) return fieldValidationResponse;
    
// 	// proceeding if nothing wrong found during validation
// 	let addCredentialFields: Field[] = [];

// 	for (const field of credentialFields) {
// 		// If field is URL some transformations are done and field data is pushed to temp state (addCredentialFields)
// 		if (field.fieldName === "URL" && field.fieldValue.length !== 0) {
// 			try {
// 				if (
// 					!field.fieldValue.startsWith("https://") &&
// 					!field.fieldValue.startsWith("http://")
// 				) {
// 					return {
// 						success: false,
// 						message: "Invalid URL",
// 					};
// 				}
// 				const domain = getDomain(field.fieldValue);
// 				addCredentialFields.push({
// 					fieldName: "Domain",
// 					fieldValue: domain,
// 					fieldType: "additional",
// 				});
// 			} catch (error) {
// 				return {
// 					success: false,
// 					message: "Invalid URL",
// 				};
// 			}
// 		}

// 		// for all others except Domain,  some transformations are done and field data is pushed to temp state (addCredentialFields)
// 		if (
// 			field.fieldName.length !== 0 && field.fieldValue.length !== 0 &&
// 			field.fieldName !== "Domain"
// 		) {
// 			const baseField: Field = {
// 				fieldName: field.fieldName,
// 				fieldValue: field.fieldValue,
// 				fieldType: field.sensitive ? "sensitive" : "meta",
// 			};

// 			//if field totp, small change need to be done
// 			if (field.fieldName === "TOTP") {
// 				if (field.fieldValue.length !== 0) {
// 					baseField.fieldType = "totp";
// 					addCredentialFields.push(baseField);
// 				} else {
//                     // This else blck checks if infact in totp field.fieldValue.length === 0, if so pushing to base fields aborted
// 					continue;
// 				}
// 			} 
		
// 			addCredentialFields.push(baseField);
			
// 		}
// 	}

// 	if(addCredentialFields.length === 0 ) return  {
// 		success: false,
// 		message: "Please add fields",
// 	};

// 	const credentialPayload = JSON.stringify({
// 		name: name,
// 		description,
// 		credentialType,
// 		credentialFields: addCredentialFields,
// 	});


// 	const response = await sendMessage("addCredential", {
// 		credentialPayload,
// 		folderId: folderId,
// 		credentialType: credentialType,
// 	});

// 	//  need to return response instead of hard coding
// 	return {
// 		success: true,
// 		message: "Credential added successfully",
// 	};
// };

import DOMPurify from 'dompurify';
import { sendMessage, getDomain } from "../utils/helper";
import {
  type CredentialFieldComponentProps,
  type Field,
  type FieldType,
  type CredentialData,
  type CredentialPayload,
  type ValidationResult
} from "../dtos/index";


const SANITIZATION_REGEX = {
	CONTROL_CHARS: /[\x00-\x1F\x7F]/g,
	MULTIPLE_SPACES: /\s{2,}/g,
	NON_BASE32: /[^A-Z2-7]/g,
	URL_PROTOCOL: /^(https?):/i,
	HTML_TAGS: /<[^>]*>?/g
  };


// TOTP validation with Base32 check
const isValidTOTP = (secret: string): boolean => {
	if (!secret) return false;
	return /^[A-Z2-7]{10,64}$/.test(secret);
  };

// URL validation with protocol check and domain extraction
const isValidURL = (url: string): { valid: boolean, domain?: string } => {
  try {
    if (!/^https?:\/\//i.test(url)) return { valid: false };
    const domain = getDomain(url);
    return { valid: true, domain };
  } catch {
    return { valid: false };
  }
};

// Main validation handler for credential fields
const validateCredentialFields = (fields: CredentialFieldComponentProps[]): ValidationResult => {
  const totpField = fields.find(f => f.fieldName === "TOTP" && f.fieldValue);
  const urlFields = fields.filter(f => f.fieldName === "URL" && f.fieldValue);

  // Validate TOTP if present
  if (totpField && !isValidTOTP(totpField.fieldValue)) {
    return { success: false, message: "Invalid TOTP secret" };
  }

  // Validate all URLs
  for (const field of urlFields) {
    const { valid, domain } = isValidURL(field.fieldValue);
    if (!valid || !domain) {
      return { success: false, message: "Invalid URL format" };
    }
  }

  return { success: true };
};

// Processes fields with proper typing and transformations
const processFields = (fields: CredentialFieldComponentProps[]): Field[] => {

  return fields.reduce<Field[]>((acc, field) => {
    if (!field.fieldName || !field.fieldValue) return acc;

    // Handle URL fields and add domain
    if (field.fieldName === "URL") {
      const { domain } = isValidURL(field.fieldValue);
      if (domain) {
        acc.push({
          fieldName: "Domain",
          fieldValue: domain,
          fieldType: "additional"
        });
      }
    }

    // Skip processing Domain fields from input
    if (field.fieldName === "Domain") return acc;

    // Determine field type
    const fieldType:  FieldType = field.fieldName === "TOTP" ? "totp" :
      field.sensitive ? "sensitive" : "meta";

    acc.push({
      fieldName: field.fieldName,
      fieldValue: field.fieldValue,
      fieldType
    });

    return acc;
  }, []);
};


const sanitizeGeneralInput = (value: string): string => {
	const trimmed = value.replace(SANITIZATION_REGEX.HTML_TAGS, '').trim();
	return DOMPurify.sanitize(trimmed, { ALLOWED_TAGS: [] });
  };

// Updated sanitization functions with proper regex references
const sanitizeTOTP = (secret: string): string => {
	return secret
	  .replace(SANITIZATION_REGEX.CONTROL_CHARS, '')
	  .replace(SANITIZATION_REGEX.MULTIPLE_SPACES, '')
	  .toUpperCase()
	  .replace(SANITIZATION_REGEX.NON_BASE32, '');
  };

  const sanitizeURL = (url: string): string => {
	if (!url) return '';
	try {
	  const cleaned = DOMPurify.sanitize(url, {
		ALLOWED_URI_REGEXP: SANITIZATION_REGEX.URL_PROTOCOL
	  });
	  if (!cleaned) throw new Error('Invalid URL');
	  
	  const parsed = new URL(cleaned);
	  parsed.protocol = 'https:';
	  return parsed.toString().replace(SANITIZATION_REGEX.CONTROL_CHARS, '');
	} catch {
	  // Better error handling
	  throw new Error('Invalid URL format');
	}
  };

// 2. Field-Specific Sanitizer --------------------------------------------------
const sanitizeCredentialField = (
  field: CredentialFieldComponentProps
): CredentialFieldComponentProps => ({
  ...field,
  fieldValue: (() => {
    const trimmed = field.fieldValue.trim();
    
    switch(field.fieldName) {
      case 'TOTP':
        return sanitizeTOTP(trimmed);
      case 'URL':
        return sanitizeURL(trimmed);
      case 'Password':
      case 'Username':
        return trimmed.replace(SANITIZATION_REGEX.CONTROL_CHARS, '');
      default:
        return sanitizeGeneralInput(trimmed);
    }
  })()
});



// 3. Main Credential Sanitization ----------------------------------------------
const sanitizeCredentialData = (
	data: CredentialData
  ): CredentialData => ({
	...data,
	name: sanitizeGeneralInput(data.name),
	description: sanitizeGeneralInput(data.description),
	credentialFields: data.credentialFields.map(sanitizeCredentialField)
  });



export const addCredentialHandler = async (
	rawCredentialData: {
    credentialFields: CredentialFieldComponentProps[];
    name: string;
    description: string;
    credentialType: string;
  },
  folderId: string
): Promise<ValidationResult> => {

  // Clean inputs first
  const credentialData = sanitizeCredentialData(rawCredentialData);

  const { credentialFields, name, description, credentialType } = credentialData;

  // Initial validation
  const validation = validateCredentialFields(credentialFields);
  if (!validation.success) return validation;

  // Process and transform fields
  const processedFields = processFields(credentialFields);
  
  if (processedFields.length === 0) {
    return { success: false, message: "Please add valid fields" };
  }

  // Prepare payload
  const payload: CredentialPayload = {
    name,
    description,
    credentialType,
    credentialFields: processedFields
  };

  try {
    await sendMessage("addCredential", {
      credentialPayload: JSON.stringify(payload),
      folderId,
      credentialType
    });
    return { success: true, message: "Credential added successfully" };
  } catch (error) {
    console.error("Failed to add credential:", error);
    return { success: false, message: "Failed to save credential" };
  }
};