// new cred definitons
import { Folder } from "./folder.dto";

export type ModifiedEvent = {
	data: {
		title: string;
		description: string;
		id: string;
		username: string;
		password: string;
		domain: string;
		url: string;
		folders: Folder[];
	};
	source: MessageEventSource;
	origin: string;
};
