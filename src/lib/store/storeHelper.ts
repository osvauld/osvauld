
import { folderStore, selectedFolder } from "./folder.store";
import { groupStore } from "./group.store";
import { credentialStore } from "./credential.store";
import { fetchAllSharedFolders } from "../apis/folder.api";
import { fetchAllUserGroups } from "../apis/group.api";
import { fetchCredentialsByFolder } from "../apis/credentials.api";
import { sendMessage } from "../components/dashboard/helper";
import { get } from 'svelte/store';
import browser from 'webextension-polyfill'

export const setFolderStore = async () => {
    const responseJson = await fetchAllSharedFolders();
    const sortedData = responseJson.data.sort((a, b) => a.name.localeCompare(b.name));
    const selectedFolderValue = get(selectedFolder);
    if (!selectedFolderValue) {
        const storedFolder = browser.storage.local.get("selectedFolder");
        storedFolder.then((value) => {
            if (value.selectedFolder) {
                for (const folder of sortedData) {
                    if (folder.id === value.selectedFolder.id) {
                        selectedFolder.set(folder);
                        return;
                    }
                }
            }
        });
    }
    folderStore.set(sortedData);
}

export const setGroupStore = async () => {
    const responseJson = await fetchAllUserGroups();
    const sortedData = responseJson.data.sort((a, b) => a.name.localeCompare(b.name));
    groupStore.set(sortedData);
}

export const setCredentialStore = async () => {
    const selectedFolderValue = get(selectedFolder);
    if (!selectedFolderValue) {
        return;
    }
    const responseJson = await fetchCredentialsByFolder(selectedFolderValue.id);
    if (!responseJson.data) {
        credentialStore.set([])
        return;
    }
    const response = await sendMessage("decryptMeta", responseJson.data);
    credentialStore.set(response.data);
}