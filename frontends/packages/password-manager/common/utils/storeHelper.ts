import { folderStore, selectedFolder, } from "../store/folder.store";
import { credentialStore } from "../store/credential.store";
import { sendMessage } from "./helper";
import { get } from "svelte/store";
import { type Folder } from "../dtos/folder.dto";
import { type User } from "../dtos/user.dto";
import { LocalStorageService } from "./storageHelper";
export const setFolderStore = async () => {
	const folderResponse = await sendMessage('getFolder', {})

	const selectedFolderValue = get(selectedFolder);
	if (!selectedFolderValue) {
		const storedFolder: any = LocalStorageService.get("selectedFolder", true);
		if (storedFolder && storedFolder != "undefined") {
			for (const folder of folderResponse) {
				if (folder.id === storedFolder.id) {
					selectedFolder.set(folder);

					return;
				}
			}
		}
	}
	folderStore.set(folderResponse);
};



export const setCredentialStore = async () => {
	const selectedFolderValue = get(selectedFolder);
	if (!selectedFolderValue) {
		return;
	}
	const responseJson = await sendMessage("getCredentialsForFolder", { folderId: selectedFolderValue.id });
	const credentials = responseJson.map((credential: any) => {
		return {
			credentialId: credential.id,
			...credential.data,
			accessType: credential.permission
		}
	})
	console.log(credentials, 'credential store')
	credentialStore.set(credentials);
	// if (!responseJson.data) {
	// 	credentialStore.set([]);
	// 	return;
	// }
	// const response = await sendMessage("decryptMeta", responseJson.data);
	// credentialStore.set(response);
};


export const getUserDetails = async (): Promise<User> => {
	const accountDetails = LocalStorageService.get("user", true);
	let user;
	if (accountDetails == null) {
		// const userJson = await getUser();
		// user = userJson.data;
		// LocalStorageService.set("user", user, true);
	} else {
		user = accountDetails;
	}
	return user;
};
