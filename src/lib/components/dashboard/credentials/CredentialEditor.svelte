<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { fly, blur } from "svelte/transition";
  import { ClosePanel, Add, BinIcon } from "../icons";
  import Loader from "../components/Loader.svelte";
  import { createEventDispatcher } from "svelte";
  import {
    selectedFolder,
    modalManager,
    DeleteConfirmationModal,
    showCredentialEditor,
  } from "../store";

  import {
    fetchFolderUsersForDataSync,
    addCredential,
    updateCredential,
    fetchCredentialUsersForDataSync,
    fetchCredentialsFieldsByIds,
  } from "../apis";

  import {
    AddCredentialPayload,
    Fields,
    User,
    AddCredentialField,
  } from "../dtos";
  import AddLoginFields from "./AddLoginFields.svelte";
  import { sendMessage } from "../helper";
  import { setCredentialStore } from "../../../store/storeHelper";
  let responseJson;
  export let edit = false;
  export let credentialId = null;
  export let description = "";
  export let name = "";
  export let credentialType = "Login";
  let notNamed = false;
  let isLoaderActive: boolean = false;
  let usersToShare: User[] = [];
  let addCredentialPaylod: AddCredentialPayload;
  let hoveredIndex: Number | null = null;
  let errorMessage = "";

  export let credentialFields = [
    { fieldName: "Username", fieldValue: "", sensitive: false },
    { fieldName: "Password", fieldValue: "", sensitive: true },
    { fieldName: "URL", fieldValue: "https://", sensitive: false },
  ];

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
    credentialFields.splice(index, 1);
    credentialFields = [...credentialFields];
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
    let domain = "";
    let addCredentialFields: Fields[] = [];
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
          const url = new URL(field.fieldValue);
          const hostname = url.hostname;
          const parts = hostname.split(".");
          if (parts.length > 2) {
            domain = parts.slice(-2).join(".");
          } else {
            domain = hostname;
          }
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
        addCredentialFields.push(baseField);
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
    if (edit) {
      if (credentialId == null) {
        throw new Error("credential not selected for edit");
      }
      await updateCredential(addCredentialPaylod, credentialId);
    } else {
      await addCredential(addCredentialPaylod);
    }
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
      ];
    } else {
      credentialFields = [
        { fieldName: "Username", fieldValue: "", sensitive: false },
        { fieldName: "Password", fieldValue: "", sensitive: true },
        { fieldName: "URL", fieldValue: "https://", sensitive: false },
      ];
    }
  };

  function closeDialog() {
    dispatcher("close");
  }

  function deleteCredential() {
    modalManager.set({
      id: credentialId,
      name: name,
      type: "Credential",
    });
    DeleteConfirmationModal.set(true);
  }

  function triggerSensitiveBubble(index: number, isEnter: boolean) {
    isEnter ? (hoveredIndex = index) : (hoveredIndex = null);
  }

  async function decryptFields(fields) {
    let fieldsForEdit = [];
    for (let field of fields) {
      const response = await sendMessage("decryptField", field.fieldValue);
      let decryptedValue = response.data;
      fieldsForEdit.push({
        fieldName: field.fieldName,
        fieldValue: decryptedValue,
        sensitive: true,
      });
    }

    return fieldsForEdit;
  }

  onMount(async () => {
    if ($showCredentialEditor) {
      responseJson = await fetchCredentialsFieldsByIds([credentialId]);
      console.log(
        "responseJson in credential Editor",
        responseJson.data[0].fields
      );
      if (responseJson && responseJson.data) {
        credentialFields = await decryptFields(responseJson.data[0].fields);
      }
      if (edit) {
        const responseJson =
          await fetchCredentialUsersForDataSync(credentialId);
        usersToShare = responseJson.data;
      } else {
        const responseJson = await fetchFolderUsersForDataSync(
          $selectedFolder.id
        );
        usersToShare = responseJson.data;
      }
    }
  });

  onDestroy(async () => {
    modalManager.set(null);
    showCredentialEditor.set(null);
  });
</script>

<form on:submit|preventDefault={saveCredential}>
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
          on:click={() => credentialTypeSelection(true)}
        >
          {edit ? "Edit login credential" : "Add Login credential"}
        </button>
        <button
          class="text-[28px] font-sans font-normal ml-8 {credentialType ===
          'Other'
            ? 'text-osvauld-quarzowhite border-b-2 border-osvauld-carolinablue'
            : 'text-osvauld-sheffieldgrey '}"
          type="button"
          on:click={() => credentialTypeSelection(false)}
        >
          {edit ? "Edit other" : "Other"}
        </button>
      </div>
      <div>
        {#if edit}
          <button
            class="bg-osvauld-frameblack p-4"
            on:click={deleteCredential}
            type="button"><BinIcon /></button
          >
        {/if}

        <button
          class="bg-osvauld-frameblack p-4"
          on:click={closeDialog}
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
          bind:value={name}
        />
        {#each credentialFields as field, index}
          {#if field.fieldName !== "Domain"}
            <AddLoginFields
              {field}
              {index}
              {hoveredIndex}
              on:select={(e) =>
                triggerSensitiveBubble(e.detail.index, e.detail.identifier)}
              on:remove={(e) => removeField(e.detail)}
            />
          {/if}
        {/each}
      </div>
      <div class="flex mr-24">
        <button
          class="py-2 m-4 bg-osvauld-addfieldgrey flex-1 flex justify-center items-center rounded-md text-osvauld-chalkwhite border-2 border-dashed border-osvauld-iconblack"
          on:click={addField}
          type="button"
        >
          <Add color={"#6E7681"} />
        </button>
      </div>
    </div>
    <div class=" mx-6 pl-3 flex justify-start items-center mb-5">
      <textarea
        rows="2"
        class="w-5/6 mt-4 h-auto min-h-[6rem] max-h-[10rem] bg-osvauld-frameblack rounded-lg scrollbar-thin border-osvauld-iconblack resize-none text-base focus:border-osvauld-iconblack focus:ring-0"
        bind:value={description}
        placeholder="Enter description about the secret"
      />
    </div>
    {#if errorMessage !== ""}
      {errorMessage}
    {/if}
    <div class="border-b border-osvauld-iconblack w-full my-2"></div>
    <div class="flex justify-end items-center mx-10 py-2">
      <button
        class="px-3 py-1.5 mb-6 whitespace-nowrap text-osvauld-fadedCancel bg-osvauld-frameblack hover:bg-osvauld-cardshade flex justify-center items-center rounded-md hover:text-osvauld-textActive text-base font-normal"
        type="button"
        on:click={closeDialog}>Cancel</button
      >
      <button
        type="submit"
        class="px-3 py-1.5 mb-6 whitespace-nowrap flex justify-center items-center ml-3 border border-osvauld-textActive text-osvauld-textActive hover:bg-osvauld-carolinablue hover:text-osvauld-frameblack hover:border-osvauld-carolinablue font-normal text-base rounded-md"
        disabled={isLoaderActive}
      >
        {#if isLoaderActive}
          <Loader size={24} color="#1F242A" duration={1} />
        {:else}
          <span>{edit ? "Save Changes" : "Add credential"}</span>
        {/if}
      </button>
    </div>
  </div>
</form>
