import {
	IntermediateCredential,
	Credential,
	ApprovedCredentialSubmitParams,
	Platform,
} from "../dtos/import.dto";

import {
	transformSafariCredentials,
	transformBitwardenCredentials,
	transformChromeCredentials,
	transformKeepassCredentials,
	transformDashlaneCredentials,
	transformFirefoxCredentials,
	transformLastpassCredentials,
	transformNordpassCredentials,
	transformOnepasswordCredentials,
	transformProtonpassCredentials,
	transformRoboformCredentials,
	finalProcessing,
} from "./parserHelper";

import Papa from "papaparse";

const manageParsedData = (platform: string, parsedData: Credential[]) => {
	//only logins are handled for now
	switch (platform) {
		case "Safari": {
			return transformSafariCredentials(parsedData);
		}
		case "Firefox": {
			return transformFirefoxCredentials(parsedData);
		}
		case "Chrome":
		case "Opera":
		case "Edge": {
			return transformChromeCredentials(parsedData);
		}
		case "Lastpass": {
			return transformLastpassCredentials(parsedData);
		}
		case "Bitwarden": {
			return transformBitwardenCredentials(parsedData);
		}

		case "Protonpass": {
			return transformProtonpassCredentials(parsedData);
		}

		case "Dashlane": {
			return transformDashlaneCredentials(parsedData);
		}

		case "Nordpass": {
			return transformNordpassCredentials(parsedData);
		}

		case "Keepass": {
			return transformKeepassCredentials(parsedData);
		}

		case "Roboform": {
			return transformRoboformCredentials(parsedData);
		}

		case "1password": {
			return transformOnepasswordCredentials(parsedData);
		}

		default: {
			console.warn(`Unsupported platform: ${platform}`);
			return [];
		}
	}
};

const papaparseWrapper = (file: File): Promise<Credential[]> => {
	return new Promise((resolve, reject) => {
		Papa.parse(file, {
			complete: (results) => {
				const parsedData = results.data as Credential[];
				resolve(parsedData);
			},
			header: true,
			skipEmptyLines: true,
			error: (error) => {
				console.error("Error parsing CSV:", error);
				reject(error);
			},
		});
	});
};

export const parseCsvLogins = async (file: File, platform: Platform) => {
	let parsedData: Credential[] = [];
	let intermediateData: IntermediateCredential[] = [];

	try {
		parsedData = await papaparseWrapper(file);
		intermediateData = manageParsedData(platform, parsedData);
	} catch (e) {
		console.error("Parsing error");
	}
	return intermediateData;
};

export const approvedCredentialSubmit = async ({
	folderId,
	...otherData
}: ApprovedCredentialSubmitParams): Promise<boolean> => {
	try {
		const operationCompletionStatus = await Promise.allSettled(
			Object.values(otherData).map((data) => finalProcessing(folderId, data)),
		);

		const operationsCompletion = operationCompletionStatus.every(
			(result) => result.status === "fulfilled" && result.value.success,
		);

		return operationsCompletion;
	} catch (error) {
		console.error("Error in approvedCredentialSubmit:", error);
		return false;
	}
};
