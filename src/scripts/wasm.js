let wasm;

const heap = new Array(128).fill(undefined);

heap.push(undefined, null, true, false);

function getObject(idx) {
	return heap[idx];
}

let heap_next = heap.length;

function dropObject(idx) {
	if (idx < 132) return;
	heap[idx] = heap_next;
	heap_next = idx;
}

function takeObject(idx) {
	const ret = getObject(idx);
	dropObject(idx);
	return ret;
}

const cachedTextDecoder =
	typeof TextDecoder !== "undefined"
		? new TextDecoder("utf-8", { ignoreBOM: true, fatal: true })
		: {
				decode: () => {
					throw Error("TextDecoder not available");
				},
			};

if (typeof TextDecoder !== "undefined") {
	cachedTextDecoder.decode();
}

let cachedUint8ArrayMemory0 = null;

function getUint8ArrayMemory0() {
	if (
		cachedUint8ArrayMemory0 === null ||
		cachedUint8ArrayMemory0.byteLength === 0
	) {
		cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
	}
	return cachedUint8ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
	ptr = ptr >>> 0;
	return cachedTextDecoder.decode(
		getUint8ArrayMemory0().subarray(ptr, ptr + len),
	);
}

function addHeapObject(obj) {
	if (heap_next === heap.length) heap.push(heap.length + 1);
	const idx = heap_next;
	heap_next = heap[idx];

	heap[idx] = obj;
	return idx;
}

let WASM_VECTOR_LEN = 0;

const cachedTextEncoder =
	typeof TextEncoder !== "undefined"
		? new TextEncoder("utf-8")
		: {
				encode: () => {
					throw Error("TextEncoder not available");
				},
			};

const encodeString =
	typeof cachedTextEncoder.encodeInto === "function"
		? function (arg, view) {
				return cachedTextEncoder.encodeInto(arg, view);
			}
		: function (arg, view) {
				const buf = cachedTextEncoder.encode(arg);
				view.set(buf);
				return {
					read: arg.length,
					written: buf.length,
				};
			};

function passStringToWasm0(arg, malloc, realloc) {
	if (realloc === undefined) {
		const buf = cachedTextEncoder.encode(arg);
		const ptr = malloc(buf.length, 1) >>> 0;
		getUint8ArrayMemory0()
			.subarray(ptr, ptr + buf.length)
			.set(buf);
		WASM_VECTOR_LEN = buf.length;
		return ptr;
	}

	let len = arg.length;
	let ptr = malloc(len, 1) >>> 0;

	const mem = getUint8ArrayMemory0();

	let offset = 0;

	for (; offset < len; offset++) {
		const code = arg.charCodeAt(offset);
		if (code > 0x7f) break;
		mem[ptr + offset] = code;
	}

	if (offset !== len) {
		if (offset !== 0) {
			arg = arg.slice(offset);
		}
		ptr = realloc(ptr, len, (len = offset + arg.length * 3), 1) >>> 0;
		const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
		const ret = encodeString(arg, view);

		offset += ret.written;
		ptr = realloc(ptr, len, offset, 1) >>> 0;
	}

	WASM_VECTOR_LEN = offset;
	return ptr;
}

function isLikeNone(x) {
	return x === undefined || x === null;
}

let cachedDataViewMemory0 = null;

function getDataViewMemory0() {
	if (
		cachedDataViewMemory0 === null ||
		cachedDataViewMemory0.buffer.detached === true ||
		(cachedDataViewMemory0.buffer.detached === undefined &&
			cachedDataViewMemory0.buffer !== wasm.memory.buffer)
	) {
		cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
	}
	return cachedDataViewMemory0;
}

function debugString(val) {
	// primitive types
	const type = typeof val;
	if (type == "number" || type == "boolean" || val == null) {
		return `${val}`;
	}
	if (type == "string") {
		return `"${val}"`;
	}
	if (type == "symbol") {
		const description = val.description;
		if (description == null) {
			return "Symbol";
		} else {
			return `Symbol(${description})`;
		}
	}
	if (type == "function") {
		const name = val.name;
		if (typeof name == "string" && name.length > 0) {
			return `Function(${name})`;
		} else {
			return "Function";
		}
	}
	// objects
	if (Array.isArray(val)) {
		const length = val.length;
		let debug = "[";
		if (length > 0) {
			debug += debugString(val[0]);
		}
		for (let i = 1; i < length; i++) {
			debug += ", " + debugString(val[i]);
		}
		debug += "]";
		return debug;
	}
	// Test for built-in
	const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
	let className;
	if (builtInMatches.length > 1) {
		className = builtInMatches[1];
	} else {
		// Failed to match the standard '[object ClassName]'
		return toString.call(val);
	}
	if (className == "Object") {
		// we're a user defined class or Object
		// JSON.stringify avoids problems with cycles, and is generally much
		// easier than looping through ownProperties of `val`.
		try {
			return "Object(" + JSON.stringify(val) + ")";
		} catch (_) {
			return "Object";
		}
	}
	// errors
	if (val instanceof Error) {
		return `${val.name}: ${val.message}\n${val.stack}`;
	}
	// TODO we could test for more things here, like `Set`s and `Map`s.
	return className;
}
/**
 */
export function init() {
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		wasm.init(retptr);
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		if (r1) {
			throw takeObject(r0);
		}
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
	}
}

/**
 * @param {string} password
 * @param {string} username
 * @returns {any}
 */
export function generate_and_encrypt_keys(password, username) {
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		const ptr0 = passStringToWasm0(
			password,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len0 = WASM_VECTOR_LEN;
		const ptr1 = passStringToWasm0(
			username,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len1 = WASM_VECTOR_LEN;
		wasm.generate_and_encrypt_keys(retptr, ptr0, len0, ptr1, len1);
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
		if (r2) {
			throw takeObject(r1);
		}
		return takeObject(r0);
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
	}
}

/**
 * @param {string} username
 * @returns {any}
 */
export function generate_keys_without_password(username) {
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		const ptr0 = passStringToWasm0(
			username,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len0 = WASM_VECTOR_LEN;
		wasm.generate_keys_without_password(retptr, ptr0, len0);
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
		if (r2) {
			throw takeObject(r1);
		}
		return takeObject(r0);
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
	}
}

/**
 * @param {string} encrypted_cert_b64
 * @param {string} salt_b64
 * @param {string} passphrase
 */
export function decrypt_and_store_keys(
	encrypted_cert_b64,
	salt_b64,
	passphrase,
) {
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		const ptr0 = passStringToWasm0(
			encrypted_cert_b64,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len0 = WASM_VECTOR_LEN;
		const ptr1 = passStringToWasm0(
			salt_b64,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len1 = WASM_VECTOR_LEN;
		const ptr2 = passStringToWasm0(
			passphrase,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len2 = WASM_VECTOR_LEN;
		wasm.decrypt_and_store_keys(retptr, ptr0, len0, ptr1, len1, ptr2, len2);
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		if (r1) {
			throw takeObject(r0);
		}
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
	}
}

/**
 * @param {string} message
 * @returns {string}
 */
export function sign_message(message) {
	let deferred3_0;
	let deferred3_1;
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		const ptr0 = passStringToWasm0(
			message,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len0 = WASM_VECTOR_LEN;
		wasm.sign_message(retptr, ptr0, len0);
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
		var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
		var ptr2 = r0;
		var len2 = r1;
		if (r3) {
			ptr2 = 0;
			len2 = 0;
			throw takeObject(r2);
		}
		deferred3_0 = ptr2;
		deferred3_1 = len2;
		return getStringFromWasm0(ptr2, len2);
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
		wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
	}
}

/**
 * @param {any} public_keys
 * @param {any} fields
 * @returns {any}
 */
export function encrypt_new_credential(public_keys, fields) {
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		wasm.encrypt_new_credential(
			retptr,
			addHeapObject(public_keys),
			addHeapObject(fields),
		);
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
		if (r2) {
			throw takeObject(r1);
		}
		return takeObject(r0);
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
	}
}

/**
 * @param {any} credentials
 * @returns {any}
 */
export function decrypt_credentials(credentials) {
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		wasm.decrypt_credentials(retptr, addHeapObject(credentials));
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
		if (r2) {
			throw takeObject(r1);
		}
		return takeObject(r0);
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
	}
}

/**
 * @param {string} encrypted_text
 * @returns {string}
 */
export function decrypt_text(encrypted_text) {
	let deferred3_0;
	let deferred3_1;
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		const ptr0 = passStringToWasm0(
			encrypted_text,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len0 = WASM_VECTOR_LEN;
		wasm.decrypt_text(retptr, ptr0, len0);
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
		var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
		var ptr2 = r0;
		var len2 = r1;
		if (r3) {
			ptr2 = 0;
			len2 = 0;
			throw takeObject(r2);
		}
		deferred3_0 = ptr2;
		deferred3_1 = len2;
		return getStringFromWasm0(ptr2, len2);
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
		wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
	}
}

/**
 * @param {any} credentials
 * @returns {any}
 */
export function decrypt_fields(credentials) {
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		wasm.decrypt_fields(retptr, addHeapObject(credentials));
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
		if (r2) {
			throw takeObject(r1);
		}
		return takeObject(r0);
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
	}
}

/**
 * @param {any} fields
 * @param {string} public_key
 * @returns {any}
 */
export function encrypt_fields(fields, public_key) {
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		const ptr0 = passStringToWasm0(
			public_key,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len0 = WASM_VECTOR_LEN;
		wasm.encrypt_fields(retptr, addHeapObject(fields), ptr0, len0);
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
		if (r2) {
			throw takeObject(r1);
		}
		return takeObject(r0);
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
	}
}

/**
 * @param {string} text
 * @returns {string}
 */
export function sign_and_hash_message(text) {
	let deferred3_0;
	let deferred3_1;
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		const ptr0 = passStringToWasm0(
			text,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len0 = WASM_VECTOR_LEN;
		wasm.sign_and_hash_message(retptr, ptr0, len0);
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
		var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
		var ptr2 = r0;
		var len2 = r1;
		if (r3) {
			ptr2 = 0;
			len2 = 0;
			throw takeObject(r2);
		}
		deferred3_0 = ptr2;
		deferred3_1 = len2;
		return getStringFromWasm0(ptr2, len2);
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
		wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
	}
}

/**
 * @param {string} field_value
 * @param {any} public_keys
 * @returns {any}
 */
export function encrypt_field_value(field_value, public_keys) {
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		const ptr0 = passStringToWasm0(
			field_value,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len0 = WASM_VECTOR_LEN;
		wasm.encrypt_field_value(retptr, ptr0, len0, addHeapObject(public_keys));
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
		if (r2) {
			throw takeObject(r1);
		}
		return takeObject(r0);
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
	}
}

/**
 * @param {any} urls
 * @returns {any}
 */
export function decrypt_urls(urls) {
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		wasm.decrypt_urls(retptr, addHeapObject(urls));
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
		if (r2) {
			throw takeObject(r1);
		}
		return takeObject(r0);
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
	}
}

/**
 * @param {string} cert_string
 * @param {string} passphrase
 * @returns {any}
 */
export function import_certificate(cert_string, passphrase) {
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		const ptr0 = passStringToWasm0(
			cert_string,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len0 = WASM_VECTOR_LEN;
		const ptr1 = passStringToWasm0(
			passphrase,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len1 = WASM_VECTOR_LEN;
		wasm.import_certificate(retptr, ptr0, len0, ptr1, len1);
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
		if (r2) {
			throw takeObject(r1);
		}
		return takeObject(r0);
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
	}
}

/**
 * @param {string} passphrase
 * @param {string} enc_pvt_key
 * @param {string} salt
 * @returns {string}
 */
export function export_certificate(passphrase, enc_pvt_key, salt) {
	let deferred5_0;
	let deferred5_1;
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		const ptr0 = passStringToWasm0(
			passphrase,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len0 = WASM_VECTOR_LEN;
		const ptr1 = passStringToWasm0(
			enc_pvt_key,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len1 = WASM_VECTOR_LEN;
		const ptr2 = passStringToWasm0(
			salt,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len2 = WASM_VECTOR_LEN;
		wasm.export_certificate(retptr, ptr0, len0, ptr1, len1, ptr2, len2);
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
		var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
		var ptr4 = r0;
		var len4 = r1;
		if (r3) {
			ptr4 = 0;
			len4 = 0;
			throw takeObject(r2);
		}
		deferred5_0 = ptr4;
		deferred5_1 = len4;
		return getStringFromWasm0(ptr4, len4);
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
		wasm.__wbindgen_free(deferred5_0, deferred5_1, 1);
	}
}

/**
 * @param {any} input
 * @returns {string}
 */
export function change_password(input) {
	let deferred2_0;
	let deferred2_1;
	try {
		const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
		wasm.change_password(retptr, addHeapObject(input));
		var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
		var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
		var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
		var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
		var ptr1 = r0;
		var len1 = r1;
		if (r3) {
			ptr1 = 0;
			len1 = 0;
			throw takeObject(r2);
		}
		deferred2_0 = ptr1;
		deferred2_1 = len1;
		return getStringFromWasm0(ptr1, len1);
	} finally {
		wasm.__wbindgen_add_to_stack_pointer(16);
		wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
	}
}

/**
 * @returns {boolean}
 */
export function is_cert_loaded() {
	const ret = wasm.is_cert_loaded();
	return ret !== 0;
}

/**
 */
export function clear_cert() {
	wasm.clear_cert();
}

function handleError(f, args) {
	try {
		return f.apply(this, args);
	} catch (e) {
		wasm.__wbindgen_exn_store(addHeapObject(e));
	}
}

async function __wbg_load(module, imports) {
	if (typeof Response === "function" && module instanceof Response) {
		if (typeof WebAssembly.instantiateStreaming === "function") {
			try {
				return await WebAssembly.instantiateStreaming(module, imports);
			} catch (e) {
				if (module.headers.get("Content-Type") != "application/wasm") {
					console.warn(
						"`WebAssembly.instantiateStreaming` failed because your server does not serve wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n",
						e,
					);
				} else {
					throw e;
				}
			}
		}

		const bytes = await module.arrayBuffer();
		return await WebAssembly.instantiate(bytes, imports);
	} else {
		const instance = await WebAssembly.instantiate(module, imports);

		if (instance instanceof WebAssembly.Instance) {
			return { instance, module };
		} else {
			return instance;
		}
	}
}

function __wbg_get_imports() {
	const imports = {};
	imports.wbg = {};
	imports.wbg.__wbindgen_object_drop_ref = function (arg0) {
		takeObject(arg0);
	};
	imports.wbg.__wbindgen_error_new = function (arg0, arg1) {
		const ret = new Error(getStringFromWasm0(arg0, arg1));
		return addHeapObject(ret);
	};
	imports.wbg.__wbindgen_string_new = function (arg0, arg1) {
		const ret = getStringFromWasm0(arg0, arg1);
		return addHeapObject(ret);
	};
	imports.wbg.__wbindgen_string_get = function (arg0, arg1) {
		const obj = getObject(arg1);
		const ret = typeof obj === "string" ? obj : undefined;
		var ptr1 = isLikeNone(ret)
			? 0
			: passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
		var len1 = WASM_VECTOR_LEN;
		getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
		getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
	};
	imports.wbg.__wbindgen_is_object = function (arg0) {
		const val = getObject(arg0);
		const ret = typeof val === "object" && val !== null;
		return ret;
	};
	imports.wbg.__wbindgen_is_undefined = function (arg0) {
		const ret = getObject(arg0) === undefined;
		return ret;
	};
	imports.wbg.__wbindgen_in = function (arg0, arg1) {
		const ret = getObject(arg0) in getObject(arg1);
		return ret;
	};
	imports.wbg.__wbindgen_is_string = function (arg0) {
		const ret = typeof getObject(arg0) === "string";
		return ret;
	};
	imports.wbg.__wbindgen_jsval_loose_eq = function (arg0, arg1) {
		const ret = getObject(arg0) == getObject(arg1);
		return ret;
	};
	imports.wbg.__wbindgen_boolean_get = function (arg0) {
		const v = getObject(arg0);
		const ret = typeof v === "boolean" ? (v ? 1 : 0) : 2;
		return ret;
	};
	imports.wbg.__wbindgen_number_get = function (arg0, arg1) {
		const obj = getObject(arg1);
		const ret = typeof obj === "number" ? obj : undefined;
		getDataViewMemory0().setFloat64(
			arg0 + 8 * 1,
			isLikeNone(ret) ? 0 : ret,
			true,
		);
		getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
	};
	imports.wbg.__wbg_String_91fba7ded13ba54c = function (arg0, arg1) {
		const ret = String(getObject(arg1));
		const ptr1 = passStringToWasm0(
			ret,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len1 = WASM_VECTOR_LEN;
		getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
		getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
	};
	imports.wbg.__wbindgen_number_new = function (arg0) {
		const ret = arg0;
		return addHeapObject(ret);
	};
	imports.wbg.__wbindgen_bigint_from_i64 = function (arg0) {
		const ret = arg0;
		return addHeapObject(ret);
	};
	imports.wbg.__wbindgen_bigint_from_u64 = function (arg0) {
		const ret = BigInt.asUintN(64, arg0);
		return addHeapObject(ret);
	};
	imports.wbg.__wbindgen_object_clone_ref = function (arg0) {
		const ret = getObject(arg0);
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_getwithrefkey_15c62c2b8546208d = function (arg0, arg1) {
		const ret = getObject(arg0)[getObject(arg1)];
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_set_20cbc34131e76824 = function (arg0, arg1, arg2) {
		getObject(arg0)[takeObject(arg1)] = takeObject(arg2);
	};
	imports.wbg.__wbg_crypto_1d1f22824a6a080c = function (arg0) {
		const ret = getObject(arg0).crypto;
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_process_4a72847cc503995b = function (arg0) {
		const ret = getObject(arg0).process;
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_versions_f686565e586dd935 = function (arg0) {
		const ret = getObject(arg0).versions;
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_node_104a2ff8d6ea03a2 = function (arg0) {
		const ret = getObject(arg0).node;
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_require_cca90b1a94a0255b = function () {
		return handleError(function () {
			const ret = module.require;
			return addHeapObject(ret);
		}, arguments);
	};
	imports.wbg.__wbindgen_is_function = function (arg0) {
		const ret = typeof getObject(arg0) === "function";
		return ret;
	};
	imports.wbg.__wbg_msCrypto_eb05e62b530a1508 = function (arg0) {
		const ret = getObject(arg0).msCrypto;
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_randomFillSync_5c9c955aa56b6049 = function () {
		return handleError(function (arg0, arg1) {
			getObject(arg0).randomFillSync(takeObject(arg1));
		}, arguments);
	};
	imports.wbg.__wbg_getRandomValues_3aa56aa6edec874c = function () {
		return handleError(function (arg0, arg1) {
			getObject(arg0).getRandomValues(getObject(arg1));
		}, arguments);
	};
	imports.wbg.__wbg_get_3baa728f9d58d3f6 = function (arg0, arg1) {
		const ret = getObject(arg0)[arg1 >>> 0];
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_length_ae22078168b726f5 = function (arg0) {
		const ret = getObject(arg0).length;
		return ret;
	};
	imports.wbg.__wbg_new_a220cf903aa02ca2 = function () {
		const ret = new Array();
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_newnoargs_76313bd6ff35d0f2 = function (arg0, arg1) {
		const ret = new Function(getStringFromWasm0(arg0, arg1));
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_new_8608a2b51a5f6737 = function () {
		const ret = new Map();
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_next_de3e9db4440638b2 = function (arg0) {
		const ret = getObject(arg0).next;
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_next_f9cb570345655b9a = function () {
		return handleError(function (arg0) {
			const ret = getObject(arg0).next();
			return addHeapObject(ret);
		}, arguments);
	};
	imports.wbg.__wbg_done_bfda7aa8f252b39f = function (arg0) {
		const ret = getObject(arg0).done;
		return ret;
	};
	imports.wbg.__wbg_value_6d39332ab4788d86 = function (arg0) {
		const ret = getObject(arg0).value;
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_iterator_888179a48810a9fe = function () {
		const ret = Symbol.iterator;
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_get_224d16597dbbfd96 = function () {
		return handleError(function (arg0, arg1) {
			const ret = Reflect.get(getObject(arg0), getObject(arg1));
			return addHeapObject(ret);
		}, arguments);
	};
	imports.wbg.__wbg_call_1084a111329e68ce = function () {
		return handleError(function (arg0, arg1) {
			const ret = getObject(arg0).call(getObject(arg1));
			return addHeapObject(ret);
		}, arguments);
	};
	imports.wbg.__wbg_new_525245e2b9901204 = function () {
		const ret = new Object();
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_self_3093d5d1f7bcb682 = function () {
		return handleError(function () {
			const ret = self.self;
			return addHeapObject(ret);
		}, arguments);
	};
	imports.wbg.__wbg_window_3bcfc4d31bc012f8 = function () {
		return handleError(function () {
			const ret = window.window;
			return addHeapObject(ret);
		}, arguments);
	};
	imports.wbg.__wbg_globalThis_86b222e13bdf32ed = function () {
		return handleError(function () {
			const ret = globalThis.globalThis;
			return addHeapObject(ret);
		}, arguments);
	};
	imports.wbg.__wbg_global_e5a3fe56f8be9485 = function () {
		return handleError(function () {
			const ret = global.global;
			return addHeapObject(ret);
		}, arguments);
	};
	imports.wbg.__wbg_set_673dda6c73d19609 = function (arg0, arg1, arg2) {
		getObject(arg0)[arg1 >>> 0] = takeObject(arg2);
	};
	imports.wbg.__wbg_isArray_8364a5371e9737d8 = function (arg0) {
		const ret = Array.isArray(getObject(arg0));
		return ret;
	};
	imports.wbg.__wbg_instanceof_ArrayBuffer_61dfc3198373c902 = function (arg0) {
		let result;
		try {
			result = getObject(arg0) instanceof ArrayBuffer;
		} catch (_) {
			result = false;
		}
		const ret = result;
		return ret;
	};
	imports.wbg.__wbg_call_89af060b4e1523f2 = function () {
		return handleError(function (arg0, arg1, arg2) {
			const ret = getObject(arg0).call(getObject(arg1), getObject(arg2));
			return addHeapObject(ret);
		}, arguments);
	};
	imports.wbg.__wbg_set_49185437f0ab06f8 = function (arg0, arg1, arg2) {
		const ret = getObject(arg0).set(getObject(arg1), getObject(arg2));
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_getTime_91058879093a1589 = function (arg0) {
		const ret = getObject(arg0).getTime();
		return ret;
	};
	imports.wbg.__wbg_new0_65387337a95cf44d = function () {
		const ret = new Date();
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_buffer_b7b08af79b0b0974 = function (arg0) {
		const ret = getObject(arg0).buffer;
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_newwithbyteoffsetandlength_8a2cb9ca96b27ec9 = function (
		arg0,
		arg1,
		arg2,
	) {
		const ret = new Uint8Array(getObject(arg0), arg1 >>> 0, arg2 >>> 0);
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_new_ea1883e1e5e86686 = function (arg0) {
		const ret = new Uint8Array(getObject(arg0));
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_set_d1e79e2388520f18 = function (arg0, arg1, arg2) {
		getObject(arg0).set(getObject(arg1), arg2 >>> 0);
	};
	imports.wbg.__wbg_length_8339fcf5d8ecd12e = function (arg0) {
		const ret = getObject(arg0).length;
		return ret;
	};
	imports.wbg.__wbg_instanceof_Uint8Array_247a91427532499e = function (arg0) {
		let result;
		try {
			result = getObject(arg0) instanceof Uint8Array;
		} catch (_) {
			result = false;
		}
		const ret = result;
		return ret;
	};
	imports.wbg.__wbg_newwithlength_ec548f448387c968 = function (arg0) {
		const ret = new Uint8Array(arg0 >>> 0);
		return addHeapObject(ret);
	};
	imports.wbg.__wbg_subarray_7c2e3576afe181d1 = function (arg0, arg1, arg2) {
		const ret = getObject(arg0).subarray(arg1 >>> 0, arg2 >>> 0);
		return addHeapObject(ret);
	};
	imports.wbg.__wbindgen_debug_string = function (arg0, arg1) {
		const ret = debugString(getObject(arg1));
		const ptr1 = passStringToWasm0(
			ret,
			wasm.__wbindgen_malloc,
			wasm.__wbindgen_realloc,
		);
		const len1 = WASM_VECTOR_LEN;
		getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
		getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
	};
	imports.wbg.__wbindgen_throw = function (arg0, arg1) {
		throw new Error(getStringFromWasm0(arg0, arg1));
	};
	imports.wbg.__wbindgen_memory = function () {
		const ret = wasm.memory;
		return addHeapObject(ret);
	};

	return imports;
}

function __wbg_init_memory(imports, memory) {}

function __wbg_finalize_init(instance, module) {
	wasm = instance.exports;
	__wbg_init.__wbindgen_wasm_module = module;
	cachedDataViewMemory0 = null;
	cachedUint8ArrayMemory0 = null;

	return wasm;
}

function initSync(module) {
	if (wasm !== undefined) return wasm;

	if (
		typeof module !== "undefined" &&
		Object.getPrototypeOf(module) === Object.prototype
	)
		({ module } = module);
	else
		console.warn(
			"using deprecated parameters for `initSync()`; pass a single object instead",
		);

	const imports = __wbg_get_imports();

	__wbg_init_memory(imports);

	if (!(module instanceof WebAssembly.Module)) {
		module = new WebAssembly.Module(module);
	}

	const instance = new WebAssembly.Instance(module, imports);

	return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
	if (wasm !== undefined) return wasm;

	if (
		typeof module_or_path !== "undefined" &&
		Object.getPrototypeOf(module_or_path) === Object.prototype
	)
		({ module_or_path } = module_or_path);
	else
		console.warn(
			"using deprecated parameters for the initialization function; pass a single object instead",
		);

	if (typeof module_or_path === "undefined") {
		module_or_path = new URL("wasm_bg.wasm", import.meta.url);
	}
	const imports = __wbg_get_imports();

	if (
		typeof module_or_path === "string" ||
		(typeof Request === "function" && module_or_path instanceof Request) ||
		(typeof URL === "function" && module_or_path instanceof URL)
	) {
		module_or_path = fetch(module_or_path);
	}

	__wbg_init_memory(imports);

	const { instance, module } = await __wbg_load(await module_or_path, imports);

	return __wbg_finalize_init(instance, module);
}

export { initSync };
export default __wbg_init;
