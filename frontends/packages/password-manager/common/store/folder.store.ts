import { writable } from "svelte/store";
import { type Folder } from "../dtos/folder.dto";
import { LocalStorageService } from "../utils/storageHelper";
export let folderStore = writable<Folder[]>([]);

export let selectedFolder = writable<Folder | undefined>(undefined);

selectedFolder.subscribe((value) => {
	if (value) {
		LocalStorageService.set("selectedFolder", value, true);
	}
});
