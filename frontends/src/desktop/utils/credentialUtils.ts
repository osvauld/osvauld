import CreditCard from "../../icons/creditCard.svelte";
import KeyIcon from "../../icons/mobileKey.svelte";
import LoginIcon from "../../icons/pwdGen.svelte";
import MobileOldKey from "../../icons/mobileOldKey.svelte";
import MobileBank from "../../icons/mobileBank.svelte";
import MobileDatabase from "../../icons/mobileDatabase.svelte";
import MobileApiIcon from "../../icons/mobileApiIcon.svelte";
import MobileNote from "../../icons/mobileNote.svelte";
import MobileContact from "../../icons/mobileContact.svelte";
import MobileWallet from "../../icons/mobileWallet.svelte";

export const CATEGORIES = [
	{ id: "logins", type: "Login", icon: LoginIcon },
	{ id: "pins", type: "PIN", icon: KeyIcon },
	{ id: "creditdebitcards", type: "Credit/DebitCard", icon: CreditCard },
	{ id: "notes", type: "Note", icon: MobileNote },
	{ id: "contacts", type: "Contact", icon: MobileContact },
	{ id: "bankaccounts", type: "Bank Account", icon: MobileBank },
	{ id: "digitalwallets", type: "Digital Wallet", icon: MobileWallet },
	{ id: "sshkeys", type: "SSH Key", icon: MobileOldKey },
	{ id: "apicredentials", type: "API Key", icon: MobileApiIcon },
	{ id: "databases", type: "Database", icon: MobileDatabase },
];

export const THEMES = {
	one: "#FAB387",
	two: "#89B4FA",
	three: "#A6E3A1",
};

export const credentialFieldsUpdater = (type: string) => {
	switch (type) {
		case "Login":
			return [
				{ fieldName: "Username", fieldValue: "", sensitive: false },
				{ fieldName: "URL", fieldValue: "https://", sensitive: false },
				{ fieldName: "Password", fieldValue: "", sensitive: true },
				{ fieldName: "TOTP", fieldValue: "", sensitive: true },
			];
		case "Credit/DebitCard":
			return [
				{ fieldName: "Name", fieldValue: "", sensitive: false },
				{ fieldName: "Card Number", fieldValue: "", sensitive: true },
				{ fieldName: "Expiration Date", fieldValue: "", sensitive: false },
				{ fieldName: "CVV", fieldValue: "", sensitive: true },
				{ fieldName: "Type", fieldValue: "", sensitive: false },
				{ fieldName: "Bank/Issuer", fieldValue: "", sensitive: false },
			];
		case "Contact":
			return [
				{ fieldName: "Name", fieldValue: "", sensitive: false },
				{ fieldName: "Email", fieldValue: "", sensitive: false },
				{ fieldName: "Phone", fieldValue: "", sensitive: false },
				{ fieldName: "Address", fieldValue: "", sensitive: false },
			];
		case "Digital Wallet":
			return [
				{ fieldName: "Wallet Name", fieldValue: "", sensitive: false },
				{ fieldName: "Wallet Address", fieldValue: "", sensitive: true },
				{ fieldName: "Private Key", fieldValue: "", sensitive: true },
			];
		case "SSH Key":
			return [
				{ fieldName: "Key Name", fieldValue: "", sensitive: false },
				{ fieldName: "Public Key", fieldValue: "", sensitive: false },
				{ fieldName: "Private Key", fieldValue: "", sensitive: true },
				{ fieldName: "Passphrase", fieldValue: "", sensitive: true },
			];
		case "Database":
			return [
				{ fieldName: "Database Name", fieldValue: "", sensitive: false },
				{ fieldName: "Host", fieldValue: "", sensitive: false },
				{ fieldName: "Port", fieldValue: "", sensitive: false },
				{ fieldName: "Username", fieldValue: "", sensitive: false },
				{ fieldName: "Password", fieldValue: "", sensitive: true },
			];
		case "PIN":
			return [
				{ fieldName: "PIN Name", fieldValue: "", sensitive: false },
				{ fieldName: "PIN", fieldValue: "", sensitive: true },
			];
		case "Note":
			return [
				{ fieldName: "Name", fieldValue: "", sensitive: false },
				{ fieldName: "Note", fieldValue: "", sensitive: false },
			];
		case "Bank Account":
			return [
				{ fieldName: "Bank Name", fieldValue: "", sensitive: false },
				{ fieldName: "Account Holder", fieldValue: "", sensitive: false },
				{ fieldName: "Account Number", fieldValue: "", sensitive: true },
				{ fieldName: "Routing Number", fieldValue: "", sensitive: true },
				{ fieldName: "Account Type", fieldValue: "", sensitive: false },
			];
		case "Social Media":
			return [
				{ fieldName: "Platform", fieldValue: "", sensitive: false },
				{ fieldName: "Username", fieldValue: "", sensitive: false },
				{ fieldName: "Password", fieldValue: "", sensitive: true },
				{ fieldName: "Profile URL", fieldValue: "", sensitive: false },
			];
		case "API Key":
			return [
				{ fieldName: "Osvauld Name", fieldValue: "", sensitive: false },
				{ fieldName: "API Name", fieldValue: "", sensitive: false },
				{ fieldName: "API Key", fieldValue: "", sensitive: true },
				{ fieldName: "API Secret", fieldValue: "", sensitive: true },
				{ fieldName: "Endpoint", fieldValue: "", sensitive: false },
				{ fieldName: "Description", fieldValue: "", sensitive: false },
			];
		default:
			return [];
	}
};

export const renderRelevantHeading = (
	credentialFields,
	credentialType,
	credentialId,
) => {
	let relevantCredentialName;
	switch (credentialType) {
		case "Login":
			relevantCredentialName =
				credentialFields.find((item) => item.fieldName === "Domain")
					?.fieldValue || credentialId;
			return relevantCredentialName;
		case "Note":
			relevantCredentialName =
				credentialFields.find((item) => item.fieldName === "Name")
					?.fieldValue || credentialId;
			return relevantCredentialName;
		case "Contact":
			relevantCredentialName =
				credentialFields.find((item) => item.fieldName === "Name")
					?.fieldValue ||
				credentialFields.find((item) => item.fieldName === "Email")
					?.fieldValue ||
				credentialId;
			return relevantCredentialName;
		case "PIN":
			relevantCredentialName =
				credentialFields.find((item) => item.fieldName === "PIN Name")
					?.fieldValue || credentialId;
			return relevantCredentialName;

		case "Credit/DebitCard":
			relevantCredentialName =
				credentialFields.find((item) => item.fieldName === "Bank/Issuer")
					?.fieldValue || credentialId;
			return relevantCredentialName;

		case "Digital Wallet":
			relevantCredentialName =
				credentialFields.find((item) => item.fieldName === "Wallet Name")
					?.fieldValue || credentialId;
			return relevantCredentialName;

		case "SSH Key":
			relevantCredentialName =
				credentialFields.find((item) => item.fieldName === "Key Name")
					?.fieldValue || credentialId;
			return relevantCredentialName;
		case "Database":
			relevantCredentialName =
				credentialFields.find((item) => item.fieldName === "Database Name")
					?.fieldValue || credentialId;
			return relevantCredentialName;
		case "Bank Account":
			relevantCredentialName =
				credentialFields.find((item) => item.fieldName === "Bank Name")
					?.fieldValue || credentialId;
			return relevantCredentialName;
		case "Social Media":
			relevantCredentialName =
				credentialFields.find((item) => item.fieldName === "Profile URL")
					?.fieldValue || credentialId;
			return relevantCredentialName;
		case "API Key":
			relevantCredentialName =
				credentialFields.find((item) => item.fieldName === "API Name")
					?.fieldValue || credentialId;
			return relevantCredentialName;
		default:
			return credentialId;
	}
};
