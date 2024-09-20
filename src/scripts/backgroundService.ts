import browser from "webextension-polyfill";
import {
	createChallenge,
	finalRegistration,
	initiateAuth,
} from "../lib/apis/auth.api";
import { fetchCredsByIds } from "../lib/apis/credentials.api";
import {
	Credential,
	CredentialFields,
	EncryptedEditField,
	Field,
	UserEncryptedFields,
	UsersForDataSync,
	CapturedCredentialData,
} from "../lib/dtos/credential.dto";
import init, {
	generate_and_encrypt_keys,
	decrypt_and_store_keys,
	sign_message,
	encrypt_new_credential,
	decrypt_credentials,
	decrypt_text,
	encrypt_fields,
	// get_pub_key,
	sign_and_hash_message,
	generate_keys_without_password,
	encrypt_field_value,
	export_certificate,
	decrypt_urls,
	decrypt_fields,
	import_certificate,
} from "./wasm";
import { User } from "../lib/dtos/user.dto.js";

type CredentialsForUsersPayload = {
	accessType?: string;
	userId: string;
	credentials: CredentialFields[];
};

type UserListForEncryption = {
	id: string;
	publicKey: string;
	accessType?: string;
};

export const initiateAuthHandler = async (
	passphrase: string,
): Promise<string> => {
	const encryptionPvtKeyObj =
		await browser.storage.local.get("encryptionPvtKey");
	const saltObj = await browser.storage.local.get("salt");
	const salt = saltObj.salt;
	const encryptionKey = encryptionPvtKeyObj.encryptionPvtKey;
	const startTime = performance.now();
	await decrypt_and_store_keys(encryptionKey, salt, passphrase);
	const pubKeyObj = await browser.storage.local.get("signPublicKey");
	console.log(
		"Time taken to decrypt and store keys:",
		performance.now() - startTime,
	);
	const challengeResponse = await createChallenge(pubKeyObj.signPublicKey);
	const signedMessage = await sign_message(challengeResponse.data.challenge);
	const verificationResponse = await initiateAuth(
		signedMessage,
		pubKeyObj.signPublicKey,
	);
	const token = verificationResponse.data.token;
	if (token) {
		await browser.storage.local.set({ token: token });
		await browser.storage.local.set({ isLoggedIn: true });
	}
	return token;
};

export const savePassphraseHandler = async (
	passphrase: string,
	challenge: string,
	username: string,
) => {
	await init();
	const keyPair = await generate_and_encrypt_keys(passphrase, username);
	await browser.storage.local.set({
		encryptionPvtKey: keyPair.get("enc_private_key"),
		signPvtKey: keyPair.get("sign_private_key"),
		encPublicKey: keyPair.get("enc_public_key"),
		signPublicKey: keyPair.get("sign_public_key"),
		salt: keyPair.get("salt"),
	});
	await decrypt_and_store_keys(
		keyPair.get("enc_private_key"),
		keyPair.get("salt"),
		passphrase,
	);

	const signature = await sign_message(challenge);
	await finalRegistration(
		username,
		signature,
		keyPair.get("sign_public_key"),
		keyPair.get("enc_public_key"),
	);
	return { isSaved: true };
};

export const decryptCredentialFieldsHandler = async (
	credentials: Credential[],
) => {
	const response: Credential[] = await decrypt_credentials(credentials);
	return response;
};

export const addCredentialHandler = async (payload: {
	users: UsersForDataSync[];
	addCredentialFields: Field[];
}): Promise<UserEncryptedFields[]> => {
	return await encrypt_new_credential(
		payload.users,
		payload.addCredentialFields,
	);
};

export const decryptFieldHandler = async (text: string): Promise<string> => {
	const decrypted = await decrypt_text(text);
	return decrypted;
};

export const encryptCredentialsForUser = async (
	credentials: CredentialFields[],
	publicKeyStr: string,
): Promise<CredentialFields[]> => {
	const encryptedCredsForUser: CredentialFields[] = [];
	for (const credential of credentials) {
		const encryptedCred: CredentialFields = {
			credentialId: credential.credentialId,
			fields: [],
		};
		encryptedCred.credentialId = credential.credentialId;
		encryptedCred.fields = await encrypt_fields(
			credential.fields,
			publicKeyStr,
		);
		encryptedCredsForUser.push(encryptedCred);
	}
	return encryptedCredsForUser;
};

export const createShareCredsPayload = async (
	creds: any,
	selectedUsers: any,
): Promise<CredentialsForUsersPayload[]> => {
	const decryptedFields = await decrypt_fields(creds);
	const userData: CredentialsForUsersPayload[] = [];
	for (const user of selectedUsers) {
		const userEncryptedFields = await encryptCredentialsForUser(
			decryptedFields,
			user.publicKey,
		);
		if (user.accessType) {
			userData.push({
				userId: user.id,
				credentials: userEncryptedFields,
				accessType: user.accessType,
			});
		} else {
			userData.push({
				userId: user.id,
				credentials: userEncryptedFields,
			});
		}
	}
	return userData;
};

export const getCertificate = async (passphrase: string) => {
	const pvtKeyObj = await browser.storage.local.get("encryptionPvtKey");
	const saltObj = await browser.storage.local.get("salt");
	try {
		const response = await export_certificate(
			passphrase,
			pvtKeyObj.encryptionPvtKey,
			saltObj.salt,
		);
		//console.log("Export response =>", response);
		return response;
		// const new_response = await import_certificate(response, "test");
		// console.log(new_response);
		// this will respond with an object with fourkeys, like encryption,sign key salt etc key and all, which can be refered from some function above.
		// Place those in the local storage to complete the prcess
	} catch (error) {
		if (error instanceof Error) {
			console.error("Error in getCertificate:", error.message);
		} else {
			console.error("An unknown error occurred in getCertificate");
		}
		return null;
	}
};

export const handlePvtKeyImport = async (
	pvtKeys: string,
	passphrase: string,
) => {
	await init();
	// const { encryptionKey, signKey, baseUrl } = JSON.parse(pvtKeys);
	// await browser.storage.local.set({ baseUrl });
	// const signPubKey = await get_pub_key(signKey);
	// const encPublicKey = await get_pub_key(encryptionKey);

	// const challegeResult = await createChallenge(signPubKey);
	// await decrypt_and_store_keys(encryptionKey, signKey, passphrase);
	// const signedMessage = await sign_message_with_stored_key(
	// 	challegeResult.data.challenge,
	// );
	// const verificationResponse = await initiateAuth(signedMessage, signPubKey);
	// const token = verificationResponse.data.token;
	// if (token) {
	// 	await browser.storage.local.set({ token: token });
	// 	await browser.storage.local.set({ isLoggedIn: true });
	// }
	// await browser.storage.local.set({
	// 	encryptionPvtKey: encryptionKey,
	// 	signPvtKey: signKey,
	// 	encPublicKey: encPublicKey,
	// 	signPublicKey: signPubKey,
	// });

	// return token;
};

export const credentialSubmitHandler = async (
	newCredential: CapturedCredentialData,
	credIds: string[],
) => {
	if (credIds.length > 0) {
		const responseJson = await fetchCredsByIds(credIds);
		const listedCredentials = responseJson.data;
		const decryptedData =
			await decryptCredentialFieldsHandler(listedCredentials);
		if (decryptedData) {
			for (const credential of decryptedData) {
				for (const field of credential.fields) {
					if (field.fieldName == "Username") {
						if (field.fieldValue == newCredential.username) {
							return null;
						}
					}
				}
			}
		}
	}

	return true;
};
export const getCurrentDomain = async () => {
	const queryOptions = { active: true, currentWindow: true };
	const tabs = await browser.tabs.query(queryOptions);
	const url = new URL(tabs[0].url!);
	return url; // Returns the domain of the current active tab
};

export const sign_hashed_message = async (message: string): Promise<string> => {
	const response = await sign_and_hash_message(message);
	return response;
};

export const generateCliKeys = async (username: string) => {
	const response = await generate_keys_without_password(username);
	const returnObj = Object.fromEntries(response);
	return returnObj;
};

export const encryptEditFields = async (data: {
	fieldValue: string;
	usersToShare: { userId: string; publicKey: string }[];
}): Promise<EncryptedEditField[]> => {
	const encryptedFields = await encrypt_field_value(
		data.fieldValue,
		data.usersToShare,
	);
	const encryptedFieldsObject = encryptedFields.map((field: any) =>
		Object.fromEntries(field),
	);
	return encryptedFieldsObject;
};

export const getDecryptedUrls = async (urls: {
	credentialId: string;
	value: string;
}): Promise<{ credentialId: string; value: string }[]> => {
	const decryptedUrls = await decrypt_urls(urls);
	return decryptedUrls;
};
