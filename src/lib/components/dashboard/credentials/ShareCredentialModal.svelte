<script lang="ts">
  import { fly } from "svelte/transition";
  import { onMount } from "svelte";

  import { showCredentialShareDrawer, modalManager } from "../store";
  import { fetchCredentialsFieldsByIds } from "../apis";

  import { User, Group, Credential, CredentialFields } from "../dtos";

  import UserGroupToggle from "../UserGroupToggle.svelte";
  import ShareCredentialsWithUser from "./ShareCredentialsWithUsers.svelte";
  import ShareCredentialsWithGroups from "./ShareCredentialsWithGroups.svelte";
  import InfoIcon from "../../basic/icons/infoIcon.svelte";
  import InfoOverlay from "../components/Info.svelte";
  import ClosePanel from "../../basic/icons/closePanel.svelte";
  export let credentials: Credential[] = [];
  export let singleCredentialId = null;
  export let users: User[];
  export let groups: Group[];
  let credentialsFields: CredentialFields[] = [];
  let showInfoTab = false;
  let infoOnHover = false;
  let saveChanges;
  let saveEnabled = false;

  const credIds = singleCredentialId
    ? [singleCredentialId]
    : credentials.map((cred) => cred.credentialId);
  onMount(async () => {
    const responseJson = await fetchCredentialsFieldsByIds(credIds);
    credentialsFields = responseJson.data;
    modalManager.set(null);
  });

  let selectedTab = "Groups";
  const toggleSelect = (e: any) => {
    selectedTab = e.detail;
  };

  /*eslint-disable*/
</script>

<div
  class="fixed top-0 right-0 z-50 flex justify-end rounded-xl blur-none"
  in:fly
  out:fly
>
  <div
    class="w-[28rem] h-screen shadow-xl translate-x-0 bg-osvauld-frameblack py-6 px-3 flex flex-col"
  >
    <div class="flex justify-between items-center p-3">
      <span
        class="font-sans text-white text-xl font-normal flex justify-between items-center"
      >
        <span>Share Credentials</span>
        <button
          class="ml-2"
          on:mouseenter={() => (infoOnHover = true)}
          on:mouseleave={() => (infoOnHover = false)}
          on:click={() => (showInfoTab = !showInfoTab)}
          ><InfoIcon color={infoOnHover ? "#BFC0CC" : "#4D4F60"} /></button
        ></span
      >
      <button class="p-2" on:click={() => showCredentialShareDrawer.set(false)}
        ><ClosePanel /></button
      >
    </div>

    {#if showInfoTab}
      <div
        class="relative h-[1.875rem] w-full px-4 py-2 mx-auto flex justify-between items-center border border-osvauld-bordergreen rounded-lg cursor-pointer mb-3 hover:bg-osvauld-bordergreen"
      >
        <span class="">
          <InfoIcon />
        </span>
        <p
          class="whitespace-nowrap text-base font-normal text-osvauld-highlightwhite
 "
        >
          Select groups/users and choose access type
        </p>

        <InfoOverlay />
      </div>
    {/if}
    <div
      class="border-b mt-2 mb-4 border-osvauld-iconblack w-[calc(100%+2.8rem)] -translate-x-3"
    ></div>

    <div
      class="flex flex-col justify-start items-center overflow-hidden h-full"
    >
      <UserGroupToggle on:select={toggleSelect} />
      {#if selectedTab === "Users"}
        <ShareCredentialsWithUser
          {users}
          {credentialsFields}
          bind:this={saveChanges}
          on:cancel={() => showCredentialShareDrawer.set(false)}
          on:enable={(e) => (saveEnabled = e.detail)}
        />
      {:else}
        <ShareCredentialsWithGroups
          {groups}
          {credentialsFields}
          bind:this={saveChanges}
          on:cancel={() => showCredentialShareDrawer.set(false)}
          on:enable={(e) => (saveEnabled = e.detail)}
        />
      {/if}
    </div>
    <div
      class="border-b mt-2 mb-4 border-osvauld-iconblack w-[calc(100%+2.8rem)] -translate-x-3"
    ></div>
    <div class="p-2 w-full flex justify-end items-center box-border mt-4">
      <button
        class="ml-auto p-2 whitespace-nowrap text-sm font-medium text-osvauld-fadedCancel"
        on:click={() => showCredentialShareDrawer.set(false)}>Cancel</button
      >

      <button
        class="ml-4 px-3 py-2 whitespace-nowrap text-sm font-medium rounded-md {saveEnabled
          ? 'bg-osvauld-carolinablue text-osvauld-frameblack border-transparent'
          : 'border border-osvauld-iconblack text-osvauld-textActive'}"
        disabled={!saveEnabled}
        on:click={() => saveChanges.shareCredentialHandler()}
        >Save changes</button
      >
    </div>
  </div>
</div>
