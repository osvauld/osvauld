import { writable } from "svelte/store";
import { Credential, SearchedCredential } from "../dtos/credential.dto";

export const credentialStore = writable<Credential[]>([]);
export const selectedCredential = writable<Credential | null>(null);
export const searchedCredential = writable<SearchedCredential | null>(null);
