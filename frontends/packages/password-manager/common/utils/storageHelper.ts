import browser from "webextension-polyfill";

const getFromStorage = async (key: string): Promise<string | undefined> => {
	try {
		const result = await browser.storage.local.get(key);
		return result[key] as string;
	} catch (error) {
		console.error(`Error getting ${key} from storage:`, error);
		return undefined;
	}
};

const setToStorage = async (key: string, value: string): Promise<void> => {
	try {
		await browser.storage.local.set({ [key]: value });
	} catch (error) {
		console.error(`Error setting ${key} in storage:`, error);
	}
};

export const StorageService = {
	getCertificate: () => getFromStorage("certificate"),
	setCertificate: (certificate: string) =>
		setToStorage("certificate", certificate),

	getToken: () => getFromStorage("token"),
	setToken: (token: string) => setToStorage("token", token),

	getBaseUrl: () => getFromStorage("baseUrl"),
	setBaseUrl: (url: string) => setToStorage("baseUrl", url),

	getIsLoggedIn: () => getFromStorage("isLoggedIn"),
	setIsLoggedIn: (isLoggedIn: string) => setToStorage("isLoggedIn", isLoggedIn),

	getEncryptedCertificate: () => getFromStorage("certificate"),
	setEncryptedCertificate: (key: string) => setToStorage("certificate", key),

	getSalt: () => getFromStorage("salt"),
	setSalt: (salt: string) => setToStorage("salt", salt),

	clearStorage: async (): Promise<void> => {
		try {
			await browser.storage.local.clear();
		} catch (error) {
			console.error("Error clearing storage:", error);
		}
	},
};
export const LocalStorageService = {
	get: <T>(key: string, parse: boolean = false): T | string | null => {
		try {
			const item = localStorage.getItem(key);
			if (item === null) return null;
			return parse ? JSON.parse(item) : item;
		} catch (error) {
			console.error(`Error getting item ${key} from localStorage:`, error);
			return null;
		}
	},

	set: <T>(key: string, value: T, stringify: boolean = false): void => {
		try {
			const item = stringify ? JSON.stringify(value) : String(value);
			localStorage.setItem(key, item);
		} catch (error) {
			console.error(`Error setting item ${key} in localStorage:`, error);
		}
	},

	remove: (key: string): void => {
		try {
			localStorage.removeItem(key);
		} catch (error) {
			console.error(`Error removing item ${key} from localStorage:`, error);
		}
	},

	clear: (): void => {
		try {
			localStorage.clear();
		} catch (error) {
			console.error("Error clearing localStorage:", error);
		}
	},
};
