<script lang="ts">
	import { onMount } from "svelte";
	import { fly, blur } from "svelte/transition";
	import { ClosePanel, Add, BinIcon } from "../icons";
	import Loader from "../components/Loader.svelte";
	import { createEventDispatcher } from "svelte";
	import {
		selectedFolder,
		modalManager,
		DeleteConfirmationModal,
	} from "../store";

	import {
		fetchFolderUsersForDataSync,
		addCredential,
		updateCredential,
		fetchCredentialUsersForDataSync,
		getEnvFieldsByCredentialId,
		getEnvsForCredential,
	} from "../apis";

	import {
		AddCredentialPayload,
		Fields,
		User,
		AddCredentialField,
	} from "../dtos";
	import AddLoginFields from "./AddLoginFields.svelte";
	import { sendMessage, getDomain } from "../helper";
	import { setCredentialStore } from "../../../store/storeHelper";

	export let edit = false;
	export let credentialFields: any = [
		{ fieldName: "Username", fieldValue: "", sensitive: false },
		{ fieldName: "Password", fieldValue: "", sensitive: true },
		{ fieldName: "URL", fieldValue: "https://", sensitive: false },
		{ fieldName: "TOTP", fieldValue: "", sensitive: true },
	];
	export let credentialId = null;
	export let description = "";
	export let name = "";
	export let credentialType = "Login";
	let notNamed = false;
	let invalidTotp = false;
	let isLoaderActive: boolean = false;
	let usersToShare: User[] = [];
	let addCredentialPaylod: AddCredentialPayload;
	let hoveredIndex: Number | null = null;
	let errorMessage = "";
	let changedFields = new Set();
	let deletedFields = [];

	const dispatcher = createEventDispatcher();
	const addField = () => {
		let newField: AddCredentialField = {
			fieldName: "",
			fieldValue: "",
			sensitive: false,
		};
		credentialFields = [...credentialFields, newField];
	};
	//TODO: change user type
	const removeField = (index: number) => {
		credentialFields[index].fieldId &&
			deletedFields.push(credentialFields[index].fieldId);
		credentialFields.splice(index, 1);
		credentialFields = [...credentialFields];
	};

	const totpValidator = (secretKey) => {
		// Base32 encoded TOTP secrets typically range in size, allowing for flexibility
		if (secretKey.length < 10 || secretKey.length > 64) {
			return false;
		}

		// Check character set: Only uppercase letters A-Z and digits 2-7 are valid
		const validBase32Regex = /^[A-Z2-7]+$/;
		if (!validBase32Regex.test(secretKey)) {
			return false;
		}

		return true;
	};
	const editCredential = async () => {
		const editedUserFields = [];
		const editedEnvFields = [];
		const newFields = [];
		const envResponse = await getEnvsForCredential(credentialId);
		const users = usersToShare.map((user) => ({
			userId: user.id,
			publicKey: user.publicKey,
		}));
		const envFieldsResponse = await getEnvFieldsByCredentialId(credentialId);

		let userEnvMap = envResponse.data.reduce((obj, item) => {
			if (!obj[item.cliUserCreatedBy]) {
				obj[item.cliUserCreatedBy] = [];
			}
			obj[item.cliUserCreatedBy].push(item);
			return obj;
		}, {});
		let domain = "";
		const envFieldMap = envFieldsResponse.data;
		for (const field of credentialFields) {
			if (field.fieldName === "URL") {
				domain = getDomain(field.fieldValue);
			}
			let editedUserField;
			if (changedFields.has(field.fieldId)) {
				editedUserField = {
					fieldId: field.fieldId,
					fieldName: field.fieldName,
					fieldType: field.fieldType,
					fieldValues: [],
				};
				if (envFieldMap[field.fieldId]) {
					// making envFieldId as userId to reuse 'encryptEditFields' case
					const envFieldPayloadForEncryption = envFieldMap[field.fieldId].map(
						(envField) => {
							return {
								userId: envField.envFieldId,
								publicKey: envField.cliUserPublicKey,
							};
						},
					);
					const response = await sendMessage("encryptEditFields", {
						fieldValue: field.fieldValue,
						usersToShare: envFieldPayloadForEncryption,
					});
					const envFieldPayload = response.data.map((envField) => {
						return {
							envFieldId: envField.userId,
							fieldValue: envField.fieldValue,
						};
					});
					editedEnvFields.push(...envFieldPayload);
				}
				const response = await sendMessage("encryptEditFields", {
					fieldValue: field.fieldValue,
					usersToShare: users,
				});
				editedUserField.fieldValues = response.data;
				editedUserFields.push(editedUserField);
			} else if (!field.fieldId) {
				const newFieldPayload = {
					fieldName: field.fieldName,
					fieldType:
						field.fieldName === "TOTP"
							? "totp"
							: field.sensitive
								? "sensitive"
								: "meta",
					fieldValues: [],
				};

				const response = await sendMessage("encryptEditFields", {
					fieldValue: field.fieldValue,
					usersToShare: users,
				});
				for (const fieldData of response.data) {
					let payload = {
						fieldValue: fieldData.fieldValue,
						userId: fieldData.userId,
						envFieldvalues: [],
					};

					if (userEnvMap[fieldData.userId]) {
						const cliUsersToShare = userEnvMap[fieldData.userId].map(
							(envData) => {
								return {
									userId: envData.envId,
									publicKey: envData.cliUserPublicKey,
								};
							},
						);
						const response = await sendMessage("encryptEditFields", {
							fieldValue: field.fieldValue,
							usersToShare: cliUsersToShare,
						});
						const envFieldsValues = response.data.map((envField) => {
							return {
								envId: envField.userId,
								fieldValue: envField.fieldValue,
							};
						});
						payload.envFieldvalues = envFieldsValues;
					}
					newFieldPayload.fieldValues.push(payload);
				}
				newFields.push(newFieldPayload);
			}
		}
		const payload = {
			name,
			description,
			credentialId,
			credentialType,
			editedUserFields,
			editedEnvFields,
			newFields,
			domain,
			deletedFields,
		};
		updateCredential(payload, credentialId);
		await setCredentialStore();
		isLoaderActive = false;
		dispatcher("close");
	};

	const saveCredential = async () => {
		isLoaderActive = true;
		errorMessage = "";
		if ($selectedFolder === null) throw new Error("folder not selected");
		if (name.length === 0) {
			isLoaderActive = false;
			notNamed = true;
			setTimeout(() => {
				notNamed = false;
			}, 1000);
			return;
		}
		if (edit) {
			await editCredential();
			return;
		}
		let totpPresence = credentialFields.filter(
			(field) => field.fieldName === "TOTP" && field.fieldValue.length !== 0,
		);
		if (totpPresence.length !== 0) {
			const isTotpValid = totpValidator(totpPresence[0].fieldValue);
			if (!isTotpValid) {
				isLoaderActive = false;
				invalidTotp = true;
				setTimeout(() => {
					invalidTotp = false;
				}, 1000);
				return;
			}
		}

		let addCredentialFields: Fields[] = [];
		let domain = "";
		for (const field of credentialFields) {
			if (field.fieldName === "URL" && field.fieldValue.length !== 0) {
				try {
					if (
						!field.fieldValue.startsWith("https://") &&
						!field.fieldValue.startsWith("http://")
					) {
						errorMessage = "Invalid URL";
						isLoaderActive = false;
						return;
					}
					domain = getDomain(field.fieldValue);
					addCredentialFields.push({
						fieldName: "Domain",
						fieldValue: domain,
						fieldType: "additional",
					});
				} catch (error) {
					errorMessage = "Invalid URL";
					isLoaderActive = false;
					return;
				}
			}
			if (
				(field.fieldName.length !== 0 || field.fieldValue.length !== 0) &&
				field.fieldName !== "Domain"
			) {
				const baseField: Fields = {
					fieldName: field.fieldName,
					fieldValue: field.fieldValue,
					fieldType: field.sensitive ? "sensitive" : "meta",
				};
				if (field.fieldName === "TOTP") {
					if (field.fieldValue.length !== 0) {
						baseField.fieldType = "totp";
						addCredentialFields.push(baseField);
					} else {
						continue;
					}
				} else {
					addCredentialFields.push(baseField);
				}
			}
		}

		addCredentialPaylod = {
			name: name,
			description: description,
			folderId: $selectedFolder.id,
			credentialType,
			userFields: [],
			domain,
		};

		const response = await sendMessage("addCredential", {
			users: usersToShare,
			addCredentialFields,
		});

		addCredentialPaylod.userFields = response;
		await addCredential(addCredentialPaylod);
		await setCredentialStore();
		isLoaderActive = false;
		dispatcher("close");
	};

	const credentialTypeSelection = (isLogin: boolean) => {
		credentialType = isLogin ? "Login" : "Other";
		if (credentialType === "Other") {
			credentialFields = [
				{
					fieldName: "Field Name",
					fieldValue: "",
					sensitive: false,
				},
				{ fieldName: "TOTP", fieldValue: "", sensitive: true },
			];
		} else {
			credentialFields = [
				{ fieldName: "Username", fieldValue: "", sensitive: false },
				{ fieldName: "Password", fieldValue: "", sensitive: true },
				{ fieldName: "URL", fieldValue: "https://", sensitive: false },
				{ fieldName: "TOTP", fieldValue: "", sensitive: true },
			];
		}
	};

	onMount(async () => {
		if (edit) {
			const responseJson = await fetchCredentialUsersForDataSync(credentialId);
			usersToShare = responseJson.data;
		} else {
			const responseJson = await fetchFolderUsersForDataSync(
				$selectedFolder.id,
			);
			usersToShare = responseJson.data;
		}
	});

	const closeDialog = () => {
		dispatcher("close");
	};

	const deleteCredential = () => {
		modalManager.set({
			id: credentialId,
			name: name,
			type: "Credential",
			private: $selectedFolder.type === "private",
		});
		DeleteConfirmationModal.set(true);
	};

	const triggerSensitiveBubble = (index: number, isEnter: boolean) => {
		isEnter ? (hoveredIndex = index) : (hoveredIndex = null);
	};

	const fieldEditHandler = (field) => {
		if (edit && field.fieldId) {
			changedFields.add(field.fieldId);
			if (field.fieldName === "URL") {
				for (const fieldData of credentialFields) {
					if (fieldData.fieldName === "Domain") {
						changedFields.add(fieldData.fieldId);
						const updatedDomain = getDomain(field.fieldValue);
						fieldData.fieldValue = updatedDomain;
					}
				}
			}
		}
	};
</script>

<form on:submit|preventDefault="{saveCredential}">
	<div
		class="bg-osvauld-frameblack rounded-3xl border border-osvauld-iconblack z-50"
		in:fly
		out:blur
	>
		<div class="flex justify-between items-center px-12 py-6">
			<div>
				<button
					class="text-[28px] font-sans font-normal {credentialType === 'Login'
						? 'text-osvauld-quarzowhite border-b-2 border-osvauld-carolinablue'
						: 'text-osvauld-sheffieldgrey '}"
					type="button"
					on:click="{() => credentialTypeSelection(true)}"
				>
					{edit ? "Edit login credential" : "Add Login credential"}
				</button>
				<button
					class="text-[28px] font-sans font-normal ml-8 {credentialType ===
					'Other'
						? 'text-osvauld-quarzowhite border-b-2 border-osvauld-carolinablue'
						: 'text-osvauld-sheffieldgrey '}"
					type="button"
					on:click="{() => credentialTypeSelection(false)}"
				>
					{edit ? "Edit other" : "Other"}
				</button>
			</div>
			<div>
				{#if edit}
					<button
						class="bg-osvauld-frameblack p-4"
						on:click="{deleteCredential}"
						type="button"><BinIcon /></button
					>
				{/if}

				<button
					class="bg-osvauld-frameblack p-4"
					on:click="{closeDialog}"
					type="button"><ClosePanel /></button
				>
			</div>
		</div>

		<div class="border-b border-osvauld-iconblack w-full"></div>

		<div class="mx-6">
			<div
				class="min-h-[32vh] max-h-[35vh] overflow-y-scroll scrollbar-thin z-50"
			>
				<input
					class="w-[78%] mb-2 mt-4 ml-6 pl-4 bg-osvauld-frameblack
           border rounded-md text-base text-osvauld-quarzowhite font-normal
           focus:border-osvauld-activeBorder flex focus:ring-0 placeholder-osvauld-iconblack {notNamed
						? 'border-red-500'
						: 'border-osvauld-iconblack '}"
					id="name"
					type="text"
					placeholder="Enter Credential name...."
					autocomplete="off"
					autofocus
					bind:value="{name}"
				/>
				{#each credentialFields as field, index}
					{#if field.fieldName !== "Domain"}
						<AddLoginFields
							{field}
							{index}
							{hoveredIndex}
							on:select="{(e) =>
								triggerSensitiveBubble(e.detail.index, e.detail.identifier)}"
							on:remove="{(e) => removeField(e.detail)}"
							on:change="{() => {
								fieldEditHandler(field);
							}}"
						/>
					{/if}
				{/each}
			</div>
			<div class="flex mr-24">
				<button
					class="py-2 m-4 bg-osvauld-addfieldgrey flex-1 flex justify-center items-center rounded-md text-osvauld-chalkwhite border-2 border-dashed border-osvauld-iconblack"
					on:click="{addField}"
					type="button"
				>
					<Add color="{'#6E7681'}" />
				</button>
			</div>
		</div>
		<div class=" mx-6 pl-3 flex justify-start items-center mb-5">
			<textarea
				rows="2"
				class="w-5/6 mt-4 h-auto min-h-[6rem] max-h-[10rem] bg-osvauld-frameblack rounded-lg scrollbar-thin border-osvauld-iconblack resize-none text-base focus:border-osvauld-iconblack focus:ring-0"
				bind:value="{description}"
				placeholder="Enter description about the secret"
			></textarea>
		</div>
		{#if errorMessage !== ""}
			{errorMessage}
		{/if}
		<div class="border-b border-osvauld-iconblack w-full my-2"></div>
		<div class="flex justify-end items-center mx-10 py-2">
			{#if invalidTotp}
				<span class="text-red-500 text-base font-normal mr-3 mb-6"
					>TOTP Secret key invalid!</span
				>
			{/if}
			<button
				class="px-3 py-1.5 mb-6 whitespace-nowrap text-osvauld-fadedCancel bg-osvauld-frameblack hover:bg-osvauld-cardshade flex justify-center items-center rounded-md hover:text-osvauld-textActive text-base font-normal"
				type="button"
				on:click="{closeDialog}">Cancel</button
			>
			<button
				type="submit"
				class="px-3 py-1.5 mb-6 whitespace-nowrap flex justify-center items-center ml-3 border border-osvauld-textActive text-osvauld-textActive hover:bg-osvauld-carolinablue hover:text-osvauld-frameblack hover:border-osvauld-carolinablue font-normal text-base rounded-md"
				disabled="{isLoaderActive}"
			>
				{#if isLoaderActive}
					<span class="w-[8.6rem] flex justify-center items-center"
						><Loader size="{24}" color="#1F242A" duration="{1}" /></span
					>
				{:else}
					<span class="w-[8.6rem]"
						>{edit ? "Save Changes" : "Add credential"}</span
					>
				{/if}
			</button>
		</div>
	</div>
</form>
