import browser from "webextension-polyfill";
import { credentialStore } from "../store/credential.store";
import { baseUrl, } from "./temp";

export const fetchCredentailsByFolder = async (folderId: string) => {
  const headers = new Headers();
  const tokenObj = await browser.storage.local.get("token");
  const token = tokenObj.token;
  headers.append("Authorization", `Bearer ${token}`);
  headers.append("Content-Type", "application/json");

  const response = await fetch(`${baseUrl}/folder/${folderId}/credential`, {
    headers,
  }).then((response) => response.json());

  if (response.data) {
    credentialStore.set(response.data);
  }
};

export const fetchCredentialById = async (credentialId: string) => {
  const headers = new Headers();
  const tokenObj = await browser.storage.local.get("token");
  const token = tokenObj.token;
  headers.append("Authorization", `Bearer ${token}`);
  headers.append("Content-Type", "application/json");

  return fetch(`${baseUrl}/credential/${credentialId}`, {
    headers,
  }).then((response) => response.json());
};

export const addCredential = async (payload: any) => {
  const headers = new Headers();
  const tokenObj = await browser.storage.local.get("token");
  const token = tokenObj.token;
  headers.append("Authorization", `Bearer ${token}`);
  headers.append("Content-Type", "application/json");

  const response = await fetch(`${baseUrl}/credential/`, {
    method: "POST",
    headers,
    body: JSON.stringify(payload),
  }).then((response) => response.json());

  return response;
};

export const fetchEncryptedCredentialsFields = async (folderId: string) => {
  const headers = new Headers();
  const tokenObj = await browser.storage.local.get("token");
  const token = tokenObj.token;
  headers.append("Authorization", `Bearer ${token}`);
  headers.append("User-Agent", "Insomnia/2023.5.7");

  const response = await fetch(`${baseUrl}/credentials/encrypted/${folderId}`, {
    method: "GET",
    headers,
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  return response.json();
};

export const shareCredential = async (shareCredential: object) => {
  const headers = new Headers();
  const tokenObj = await browser.storage.local.get("token");
  const token = tokenObj.token;
  headers.append("Authorization", `Bearer ${token}`);
  headers.append("Content-Type", "application/json");
  headers.append("User-Agent", "Insomnia/2023.5.7");

  const response = await fetch(`${baseUrl}/credentials/`, {
    method: "PUT",
    headers,
    body: JSON.stringify(shareCredential),
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  return response.json();
};

export const fetchEncryptedFieldsByIds = async (credentialIds: string[]) => {
  const headers = new Headers();
  const tokenObj = await browser.storage.local.get("token");
  const token = tokenObj.token;
  headers.append("Authorization", `Bearer ${token}`);
  headers.append("Content-Type", "application/json");

  const response = await fetch(`${baseUrl}/credentials/encrypted/`, {
    method: "POST",
    headers,
    body: JSON.stringify({ credentialIds }),
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  return response.json();
};
