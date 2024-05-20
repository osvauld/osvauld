import { ShareFolderWithUsersPayload, ShareFolderWithGroupsPayload } from "../dtos/request.dto";
import { FolderGroupResponse, FolderUserResponse, BaseResponse, FetchAllFoldersResponse } from "../dtos/response.dto";
import { getTokenAndBaseUrl, sendMessage } from "../components/dashboard/helper";

export const fetchAllSharedFolders = async (): Promise<FetchAllFoldersResponse> => {

  const { token, baseUrl } = await getTokenAndBaseUrl()
  const options = {
    method: "GET",
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
    },
  };
  return fetch(`${baseUrl}/folders/shared`, options).then(
    (response) => response.json()
  );
};

export const fetchFolderUsers = async (folderId: string): Promise<FolderUserResponse> => {
  const headers = new Headers();
  const { token, baseUrl } = await getTokenAndBaseUrl()
  headers.append("Authorization", `Bearer ${token}`);
  headers.append("Content-Type", "application/json");

  return fetch(`${baseUrl}/folder/${folderId}/users`, {
    headers,
  }).then((response) => response.json());
};

export const fetchFolderUsersForDataSync = async (folderId: string): Promise<any> => {
  const headers = new Headers();
  const { token, baseUrl } = await getTokenAndBaseUrl()
  headers.append("Authorization", `Bearer ${token}`);
  headers.append("Content-Type", "application/json");

  return fetch(`${baseUrl}/folder/${folderId}/users-data-sync`, {
    headers,
  }).then((response) => response.json());

}

export const fetchFolderGroups = async (folderId: string): Promise<FolderGroupResponse> => {
  const headers = new Headers();
  const { token, baseUrl } = await getTokenAndBaseUrl()
  headers.append("Authorization", `Bearer ${token}`);
  headers.append("Content-Type", "application/json");

  return await fetch(`${baseUrl}/folder/${folderId}/groups`, {
    headers,
  }).then((response) => response.json());
};

export const createFolder = async (payload: any): Promise<BaseResponse> => {
  const headers = new Headers();
  const { token, baseUrl } = await getTokenAndBaseUrl()
  headers.append("Authorization", `Bearer ${token}`);
  headers.append("Content-Type", "application/json");

  const response = await fetch(`${baseUrl}/folder/`, {
    method: "POST",
    headers,
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  return response.json();
};

export const renameFolder = async (payload: any, id: string): Promise<BaseResponse> => {
  const headers = new Headers();
  const { token, baseUrl } = await getTokenAndBaseUrl()
  headers.append("Authorization", `Bearer ${token}`);
  const response = await fetch(`${baseUrl}/folder/${id}`, {
    method: "PUT",
    headers,
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  return response.json();
};

export const shareFolderWithUsers = async (payload: ShareFolderWithUsersPayload) => {
  const headers = new Headers();
  const signatureResponse = await sendMessage("hashAndSign", { message: JSON.stringify(payload) });
  headers.append('Signature', signatureResponse.signature)
  const { token, baseUrl } = await getTokenAndBaseUrl()
  headers.append("Authorization", `Bearer ${token}`);
  headers.append("Content-Type", "application/json");

  const response = await fetch(`${baseUrl}/share-folder/users`, {
    method: "POST",
    headers,
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  return response.json();
};

export const shareFolderWithGroups = async (payload: ShareFolderWithGroupsPayload) => {
  const headers = new Headers();
  const signatureResponse = await sendMessage("hashAndSign", { message: JSON.stringify(payload) });
  headers.append('Signature', signatureResponse.signature)
  const { token, baseUrl } = await getTokenAndBaseUrl()
  headers.append("Authorization", `Bearer ${token}`);
  headers.append("Content-Type", "application/json");

  const response = await fetch(`${baseUrl}/share-folder/groups`, {
    method: "POST",
    headers,
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  return response.json();
}

export const editFolderPermissionForUser = async (folderId: string, userId: string, accessType: string) => {
  const headers = new Headers();
  const { token, baseUrl } = await getTokenAndBaseUrl()
  const signatureResponse = await sendMessage("hashAndSign", { message: JSON.stringify({ accessType, userId }) });
  headers.append('Signature', signatureResponse.signature)
  headers.append("Authorization", `Bearer ${token}`);
  headers.append("Content-Type", "application/json");

  const response = await fetch(`${baseUrl}/folder/${folderId}/edit-user-access`, {
    method: "POST",
    headers,
    body: JSON.stringify({ accessType, userId }),
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  return response.json();
}


export const editFolderPermissionForGroup = async (folderId: string, groupId: string, accessType: string) => {
  const headers = new Headers();
  const signatureResponse = await sendMessage("hashAndSign", { message: JSON.stringify({ accessType, groupId }) });
  headers.append('Signature', signatureResponse.signature)
  const { token, baseUrl } = await getTokenAndBaseUrl()
  headers.append("Authorization", `Bearer ${token}`);
  headers.append("Content-Type", "application/json");

  const response = await fetch(`${baseUrl}/folder/${folderId}/edit-group-access`, {
    method: "POST",
    headers,
    body: JSON.stringify({ accessType, groupId }),
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  return response.json();
}

export const removeFolder = async (folderId: string) => {

  const signatureResponse = await sendMessage("hashAndSign", { message: folderId });
  const { token, baseUrl } = await getTokenAndBaseUrl()
  const headers = new Headers();
  headers.append("Authorization", `Bearer ${token}`);
  headers.append("Content-Type", "application/json");
  headers.append('Signature', signatureResponse.signature)
  const response = await fetch(`${baseUrl}/folder/${folderId}`, {
    method: "DELETE",
    headers,
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  return response.json();
}

