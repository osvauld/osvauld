import { emit, listen } from "@tauri-apps/api/event";
import { Doc, DocCollection, Job } from "@blocksuite/store";
import { sendMessage } from "@osvauld/password-manager-common";
import * as Y from "yjs";

// In tauriSync.ts
export class TauriSync {
	private collection: DocCollection;
	private deviceId: string;
	private job: Job;
	private currentDoc: Doc | null;

	constructor(collection: DocCollection, deviceId: string) {
		this.collection = collection;
		this.deviceId = deviceId;
		this.job = new Job({ collection });
		this.currentDoc = null;

		console.log("TauriSync: Initializing with deviceId:", deviceId);
		this.initializeDocument();
		this.setupTauriListeners().catch((error) => {
			console.error("TauriSync: Error setting up Tauri listeners:", error);
		});
	}

	private initializeDocument() {
		this.currentDoc = this.collection.getDoc("page1");
		if (!this.currentDoc) {
			this.collection.createDoc({ id: "page1" });
			this.currentDoc = this.collection.getDoc("page1");
		}

		if (this.currentDoc) {
			// Set up update handler for the document
			this.currentDoc.spaceDoc.on(
				"update",
				(update: Uint8Array, origin: any) => {
					// Only broadcast updates that originated from this device
					if (origin !== "remote") {
						console.log("TauriSync: Local doc update detected, sending update");
						this.handleDocUpdate(update);
					}
				},
			);
		}
	}

	private async handleDocUpdate(update: Uint8Array) {
		try {
			// Convert the update to a regular array for serialization
			const updateArray = Array.from(update);
			console.log("TauriSync: Sending update, length:", updateArray.length);

			// Emit the update event
			await emit("sync-update", JSON.stringify(updateArray));
			console.log("TauriSync: Successfully sent update");
		} catch (error) {
			console.error("TauriSync: Error sending update:", error);
		}
	}

	public async sendInitialSnapshot() {
		console.log("TauriSync: Sending initial Y.js state");
		if (!this.currentDoc) {
			console.error("TauriSync: No document available to create snapshot");
			return;
		}

		try {
			const encodedState = Y.encodeStateAsUpdate(this.currentDoc.spaceDoc);
			const stateArray = Array.from(encodedState);
			console.log("TauriSync: Sending snapshot, length:", stateArray.length);

			await sendMessage("sendSnapshot", stateArray);
			console.log("TauriSync: Successfully sent initial snapshot");
		} catch (error) {
			console.error("TauriSync: Error sending initial snapshot:", error);
		}
	}

	private async setupTauriListeners() {
		console.log("TauriSync: Setting up Tauri listeners");
		try {
			await listen("sync-snapshot-be", async (event) => {
				console.log("TauriSync: Received sync-snapshot-be event");
				try {
					if (!event.payload) throw new Error("Received null payload");
					const binaryData = new Uint8Array(event.payload as number[]);
					await this.loadSnapshot(binaryData);
				} catch (error) {
					console.error("TauriSync: Error processing sync snapshot:", error);
				}
			});

			await listen("sync-update-be", async (event) => {
				console.log("TauriSync: Received sync-update-be event");
				try {
					if (!event.payload) throw new Error("Invalid payload");
					const update = new Uint8Array(event.payload as number[]);
					await this.applyUpdate(update);
				} catch (error) {
					console.error("TauriSync: Error processing sync update:", error);
				}
			});
		} catch (error) {
			console.error("TauriSync: Error in setupTauriListeners:", error);
			throw error;
		}
	}

	private async loadSnapshot(snapshot: Uint8Array) {
		if (!this.currentDoc) {
			console.error("TauriSync: No document available to load snapshot");
			return;
		}

		try {
			Y.applyUpdate(this.currentDoc.spaceDoc, snapshot, "remote");
			console.log("TauriSync: Successfully loaded snapshot");
		} catch (error) {
			console.error("TauriSync: Error loading snapshot:", error);
		}
	}

	private async applyUpdate(update: Uint8Array) {
		if (!this.currentDoc) {
			console.error("TauriSync: No document available to apply update");
			return;
		}

		try {
			Y.applyUpdate(this.currentDoc.spaceDoc, update, "remote");
			console.log("TauriSync: Successfully applied update");
		} catch (error) {
			console.error("TauriSync: Error applying update:", error);
		}
	}
}
