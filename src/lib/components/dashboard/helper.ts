import browser from "webextension-polyfill";
type TypeToClassKey = "reader" | "manager";

export const setbackground = (type: TypeToClassKey): string => {
	const typeToClassMap: Record<TypeToClassKey, string> = {
		reader: "bg-osvauld-readerOrange text-osvauld-readerText",
		manager: "bg-osvauld-managerPurple text-osvauld-managerText",
	};

	return typeToClassMap[type] || "";
};

export const getTokenAndBaseUrl = async () => {
	const [token, baseUrl] = await Promise.all([
		browser.storage.local.get("token"),
		browser.storage.local.get("baseUrl"),
	]);
	return { token: token.token, baseUrl: baseUrl.baseUrl };
};

export const sendMessage = async (action: string, data: any = {}) => {
	try {
		const response = await browser.runtime.sendMessage({
			action,
			data,
		});
		return response;
	} catch (error) {
		console.error("Error sending message:", error);
	}
};
export const searchObjects = (query, objects) => {
	const searchResults = [];

	for (const obj of objects) {
		for (const prop in obj) {
			if (
				typeof obj[prop] === "string" &&
				obj[prop].toLowerCase().includes(query.toLowerCase())
			) {
				searchResults.push(obj);
				break;
			}
		}
	}

	return searchResults;
};

export const clickOutside = (node) => {
	const handleClick = (event) => {
		if (node && !node.contains(event.target) && !event.defaultPrevented) {
			node.dispatchEvent(new CustomEvent("clickedOutside", node));
		}
	};

	document.addEventListener("click", handleClick, true);

	return {
		destroy() {
			document.removeEventListener("click", handleClick, true);
		},
	};
};

export const generatePassword = (length: number) => {
	const lowercaseChars = "abcdefghijklmnopqrstuvwxyz";
	const uppercaseChars = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
	const numberChars = "0123456789";
	const symbolChars = "!@#$%^&*()_";

	const charSet = lowercaseChars + uppercaseChars + numberChars + symbolChars;
	const passwordArray = new Uint8Array(length); // For storing random bytes
	window.crypto.getRandomValues(passwordArray);
	let password = "";
	for (let i = 0; i < length; i++) {
		const randomIndex = passwordArray[i] % charSet.length;
		password += charSet[randomIndex];
	}
	return password;
};

export function debounce(func, delay) {
	let timeoutId;
	return function (...args) {
		clearTimeout(timeoutId);
		timeoutId = setTimeout(() => {
			func.apply(this, args);
		}, delay);
	};
}

type TransformedPayload = {
	name: string;
	description: string;
	folderId: string;
	credentialType: string;
	domain: string;
	fields: {
		fieldName?: string;
		fieldType?: string;
		fieldValues: {
			userId: string;
			fieldValue: string;
		}[];
	}[];
};

export const transformAddCredentialPayload = (
	payload: any,
): TransformedPayload => {
	const fieldsMap: {
		[key: string]: {
			fieldName?: string;
			fieldType?: string;
			fieldValues: { userId: string; fieldValue: string }[];
		};
	} = {};

	payload.userFields.forEach((userField) => {
		userField.fields.forEach((field) => {
			const key = field.fieldName;
			if (!fieldsMap[key]) {
				fieldsMap[key] = {
					fieldName: field.fieldName,
					fieldType: field.fieldType,
					fieldValues: [],
				};
			}
			fieldsMap[key].fieldValues.push({
				userId: userField.userId,
				fieldValue: field.fieldValue,
			});
		});
	});

	const transformedFields = Object.values(fieldsMap);

	return {
		name: payload.name,
		description: payload.description,
		folderId: payload.folderId,
		credentialType: payload.credentialType,
		domain: payload.domain,
		fields: transformedFields,
	};
};

export const getDomain = (urlString: string) => {
	const url = new URL(urlString);
	const hostname = url.hostname;
	const parts = hostname.split(".");
	let domain;
	if (parts.length > 2) {
		domain = parts.slice(-2).join(".");
	} else {
		domain = hostname;
	}
	return domain;
};
