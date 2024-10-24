<script lang="ts">
	import { onMount } from "svelte";
	import { fly } from "svelte/transition";
	import {
		fetchGroupUsers,
		addUserToGroup,
		fetchCredentialFieldsByGroupId,
		fetchUsersWithoutGroupAccess,
	} from "../apis";
	import { CredentialFields } from "../dtos";

	import { User } from "../dtos";
	import { ClosePanel, Lens, UserCheck, UserPlus } from "../icons";
	import {
		selectedGroup,
		showAddUserToGroupDrawer,
		groupUsers,
		toastStore,
	} from "../store";
	import RoleSelector from "../components/roleSelector.svelte";
	import { sendMessage } from "../helper";

	let users: User[] = [];
	let userDataForApproval: User;
	let searchInput = "";
	let hoveredIndex = null;
	let selectedPermission = null;
	let selectedUserIndice = null;
	let roleSelectionPrompt = false;
	let credentialFields: CredentialFields[] = [];
	onMount(async () => {
		if (!$selectedGroup) return;
		const userGroup = await fetchUsersWithoutGroupAccess(
			$selectedGroup.groupId,
		);
		users = userGroup.data;
		const credentialFieldsResponse = await fetchCredentialFieldsByGroupId(
			$selectedGroup.groupId,
		);
		credentialFields = credentialFieldsResponse.data;
	});

	const approveSelections = async () => {
		if (!selectedPermission) return;
		const userData = await sendMessage("createShareCredPayload", {
			creds: credentialFields,
			users: [userDataForApproval],
		});
		const payload = {
			groupId: $selectedGroup.groupId,
			memberId: userDataForApproval.id,
			memberRole: selectedPermission,
			credentials: userData[0].credentials,
		};
		const userAdditionResponse = await addUserToGroup(payload);
		selectedUserIndice = null;
		if (userAdditionResponse.success) {
			toastStore.set({
				type: true,
				message: userAdditionResponse.message,
				show: true,
			});
		}
		users = users.filter((u) => u.id !== userDataForApproval.id);
	};

	const addUsertoGroup = async (user: User, selectedUser: number) => {
		roleSelectionPrompt = true;
		selectedUserIndice = selectedUser;
		userDataForApproval = user;
		if ($selectedGroup === null) {
			throw new Error("Group not selected");
		}
	};

	const selectionMaker = async (e) => {
		selectedPermission = e.detail.permission;
		roleSelectionPrompt = false;
		await approveSelections();
	};

	const closeDrawer = () => {
		showAddUserToGroupDrawer.set(false);
		fetchGroupUsers($selectedGroup.groupId).then((usersResponse) => {
			groupUsers.set(usersResponse.data);
		});
	};

	const handleCurserMoveOut = () => {
		selectedUserIndice = null;
	};
</script>

<div
	class="fixed top-0 right-0 z-50 blur-none flex justify-end rounded-xl"
	in:fly
	out:fly
>
	<div
		class="w-[30vw] h-screen shadow-xl translate-x-0 bg-osvauld-frameblack p-6"
	>
		<div class="flex justify-between items-center p-3">
			<span class="font-sans text-white text-28 font-normal">Add Users</span>
			<button class="p-2" on:click="{closeDrawer}"><ClosePanel /></button>
		</div>
		<div class="border border-osvauld-bordergreen mb-2 w-full"></div>
		<div
			class="flex-grow max-h-[85vh]"
			on:mouseleave="{handleCurserMoveOut}"
			role="complementary"
		>
			<div class="p-2 border border-osvauld-bordergreen rounded-lg">
				<div
					class="h-[1.875rem] w-full px-2 mx-auto flex justify-start items-center border border-osvauld-bordergreen rounded-lg cursor-pointer"
				>
					<Lens />
					<input
						type="text"
						bind:value="{searchInput}"
						class="h-[1.60rem] w-full bg-osvauld-frameblack border-0 text-osvauld-quarzowhite placeholder-osvauld-placeholderblack border-transparent text-base focus:border-transparent focus:ring-0 cursor-pointer"
						placeholder="Search for users"
					/>
				</div>

				<div class="border border-osvauld-bordergreen my-1 w-full mb-1"></div>

				<div
					class="overflow-y-scroll scrollbar-thin bg-osvauld-frameblack w-full min-h-[40vh] max-h-[70vh]"
				>
					{#each users as user, index}
						<li
							class="list-none my-1 py-1 mr-1 border border-osvauld-bordergreen rounded-lg hover:bg-osvauld-bordergreen"
							on:mouseenter="{() => (hoveredIndex = index)}"
							on:mouseleave="{() => (hoveredIndex = null)}"
						>
							<div class="flex items-center justify-between px-3">
								<span class="text-base">{user.name}</span>
								<button
									class="p-2 relative"
									on:click="{() => addUsertoGroup(user, index)}"
								>
									{#if selectedUserIndice === index}
										<svelte:component
											this="{roleSelectionPrompt ? RoleSelector : UserCheck}"
											on:select="{selectionMaker}"
										/>
									{:else}
										<span
											class="{hoveredIndex === index ? 'visible' : 'invisible'}"
										>
											<UserPlus />
										</span>
									{/if}
								</button>
							</div>
						</li>
					{/each}
				</div>
			</div>
		</div>
	</div>
</div>
