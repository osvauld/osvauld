import browser from "webextension-polyfill";
import { SearchedCredential } from "../dtos/credential.dto";
import { StorageService } from "../utils/storageHelper";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
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
		StorageService.getToken(),
		StorageService.getBaseUrl(),
	]);
	return { token, baseUrl };
};

export const sendMessage = async (action: string, data: any = {}) => {
	try {
		const response = await browser.runtime.sendMessage({
			action,
			data,
		});
		return response;
	} catch (error) {
		console.error(
			"Error sending message:",
			error,
			"\ndata",
			data,
			"action\n",
			action,
		);
	}
};
export const searchObjects = (
	query: string,
	credentials: SearchedCredential[],
) => {
	const searchResults: SearchedCredential[] = [];

	for (const credential of credentials) {
		for (const prop in credential) {
			if (
				typeof credential[prop as keyof SearchedCredential] === "string" &&
				credential[prop as keyof SearchedCredential]
					.toLowerCase()
					.includes(query.toLowerCase())
			) {
				searchResults.push(credential);
				break;
			}
		}
	}

	return searchResults;
};

export const clickOutside = (node: any) => {
	const handleClick = (event: any) => {
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



export const getDomain = (urlString: string) => {
	let url: URL;
	try {
		url = new URL(urlString);
	} catch (_) {
		throw new Error("Invalid URL");
	}
	const hostname = url.hostname;
	const parts = hostname.split(".");
	let domain: string | undefined;
	if (parts.length > 2) {
		domain = parts.slice(-2).join(".");
	} else {
		domain = hostname;
	}
	return domain;
};

export const writeToClipboard = async (text: string) => {
	const tauriEnv = isTauri();
	if (tauriEnv) {
		try {
			await writeText(text);
		} catch (error) {
			console.error("Error writing to clipboard:", error);
		}
	} else {
		navigator.clipboard.writeText(text);
	}
};

function isTauri() {
	// @ts-ignore
	return typeof window.__TAURI__ !== "undefined";
}
