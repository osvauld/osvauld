import { writable } from 'svelte/store';
import { WSConnection } from '../utils/wsConnector';

export const wsConnector = writable(new WSConnection());