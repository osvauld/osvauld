import { sendMessage } from "@osvauld/password-manager-common";
import { invoke } from "@tauri-apps/api/core";

interface RegisterMessage {
	action: "Register";
	payload: {
		user_id: string;
	};
}

interface GetConnectionStringResponseMessage {
	action: "GetConnectionStringResponse";
	payload: {
		user_id: string;
		reciever_ws_connection_id: string;
		connection_string: string;
	};
}

interface GetConnectionRequestMessage {
	action: "GetConnectionRequest";
	payload: {
		reciever_ws_connection_id: string;
	};
}

export interface GetConnectionStringRequestMessage {
	action: "GetConnectionStringRequest";
	payload: {
		user_id: string;
	};
}

interface ConnectionStringResponseMessage {
	action: "ConnectionStringResponse";
	payload: {
		user_id: string;
		connection_string: string;
	};
}

type WSMessage =
	| RegisterMessage
	| GetConnectionStringRequestMessage
	| GetConnectionRequestMessage
	| GetConnectionStringResponseMessage
	| ConnectionStringResponseMessage;

export class WSConnection {
	private socket: WebSocket;

	constructor() {
		this.socket = new WebSocket("wss://osvauld-tscs.onrender.com/ws");

		this.socket.addEventListener("open", (event) => {
			console.log("Connection established:", event);
		});

		this.socket.addEventListener("message", (event) => {
			this.handleSocketMessage(event, this.socket);
		});

		this.socket.addEventListener("close", (event) => {
			console.log("Connection closed:", event.reason);
		});

		this.socket.addEventListener("error", (event) => {
			console.error("Connection error:", event);
		});
	}

	public sendWSMessage(message: WSMessage): void {
		if (this.socket.readyState === WebSocket.OPEN) {
			this.socket.send(JSON.stringify(message));
			console.log("Message sent:", message);
		} else {
			console.error("Cannot send message: Connection is not open.");
		}
	}

	public sendRegisterMessage(userId: string): void {
		const message: RegisterMessage = {
			action: "Register",
			payload: { user_id: userId },
		};
		this.sendWSMessage(message);
		console.log("Registration completed for:", userId);
	}

	public sendConnectionStringRequest(userId: string): Promise<string> {
		return new Promise<string>((resolve, reject) => {
			const handleResponse = (event: MessageEvent) => {
				this.socket.removeEventListener("message", handleResponse);
				const message: WSMessage = JSON.parse(event.data);
				let connection_string = "";
				switch (message.action) {
					case "ConnectionStringResponse":
						connection_string = message.payload.connection_string;
						break;
					default:
						console.error("Unknown message action:", message);
				}
				resolve(connection_string);
			};

			this.socket.addEventListener("message", handleResponse);

			if (this.socket.readyState === WebSocket.OPEN) {
				const getConnectionStringRequestMessage: GetConnectionStringRequestMessage =
					{
						action: "GetConnectionStringRequest",
						payload: {
							user_id: userId,
						},
					};
				const request = JSON.stringify(getConnectionStringRequestMessage);
				this.socket.send(request);
				console.log("Request sent:", request);
			} else {
				this.socket.removeEventListener("message", handleResponse);
				reject(new Error("Connection is not open."));
			}
		});
	}

	public handleSocketMessage(
		event: MessageEvent,
		socketConnection: WebSocket,
	): void {
		try {
			const message: WSMessage = JSON.parse(event.data);
			console.log("Recieved message: ", message);
			switch (message.action) {
				case "GetConnectionRequest":
					sendMessage("getTicket")
						.then(async (ticket: string) => {
							await invoke("start_p2p_listener");
							const connectionResponse: GetConnectionStringResponseMessage = {
								action: "GetConnectionStringResponse",
								payload: {
									user_id: "user-id",
									reciever_ws_connection_id:
										message.payload.reciever_ws_connection_id,
									connection_string: ticket,
								},
							};

							if (socketConnection.readyState === WebSocket.OPEN) {
								socketConnection.send(JSON.stringify(connectionResponse));
								console.log("Request sent:", connectionResponse);
							} else {
								console.error("Connection lost couldnt sent message");
							}
						})
						.catch((err) => {
							console.error("Error creating connection ticket", err);
						});
					break;

				default:
					console.error("Unknown message action:", message);
			}
		} catch (error) {
			console.error("Error handling message:", error);
			if (error instanceof SyntaxError) {
				console.error("Invalid JSON in message:", event.data);
			}
		}
	}

	public closeConnection(): void {
		this.socket.close();
	}
}
