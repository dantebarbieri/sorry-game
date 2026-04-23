export interface CreateRoomRequest {
	player_name: string;
	num_players: number;
	rules?: string;
}
export interface CreateRoomResponse {
	room_code: string;
	session_token: string;
	player_index: number;
}

export interface JoinRoomRequest {
	player_name: string;
}
export interface JoinRoomResponse {
	session_token: string;
	player_index: number;
}

export interface RoomInfoResponse {
	room_code: string;
	num_players: number;
	rules: string;
	players_joined: number;
	phase: string;
}

export interface MetaResponse {
	available_rules: string[];
	available_strategies: string[];
}

export const DEFAULT_SERVER_URL =
	(typeof import.meta !== 'undefined' && (import.meta as unknown as { env?: { VITE_SERVER_URL?: string } }).env?.VITE_SERVER_URL) ||
	'http://localhost:8080';

async function rpc<T>(method: string, path: string, body?: unknown, base: string = DEFAULT_SERVER_URL): Promise<T> {
	let res: Response;
	try {
		res = await fetch(`${base}${path}`, {
			method,
			headers: body ? { 'content-type': 'application/json' } : undefined,
			body: body ? JSON.stringify(body) : undefined
		});
	} catch (e) {
		throw new Error(
			`Could not reach ${base}. Start sorry-server (\`cd sorry-server && cargo run\`) or update the Server URL. (${e instanceof Error ? e.message : e})`
		);
	}
	if (!res.ok) {
		const text = await res.text().catch(() => '');
		throw new Error(`${method} ${path} → ${res.status} ${text}`);
	}
	return (await res.json()) as T;
}

export function createRoom(req: CreateRoomRequest, base?: string) {
	return rpc<CreateRoomResponse>('POST', '/api/rooms', req, base);
}

export function joinRoom(code: string, req: JoinRoomRequest, base?: string) {
	return rpc<JoinRoomResponse>('POST', `/api/rooms/${code}/join`, req, base);
}

export function roomInfo(code: string, base?: string) {
	return rpc<RoomInfoResponse>('GET', `/api/rooms/${code}`, undefined, base);
}

export function meta(base?: string) {
	return rpc<MetaResponse>('GET', '/api/meta', undefined, base);
}

/** Convert an HTTP base URL to the WebSocket URL for a room. */
export function wsUrl(base: string, code: string, token: string): string {
	const wsBase = base.replace(/^http/, 'ws');
	return `${wsBase}/api/rooms/${code}/ws?token=${encodeURIComponent(token)}`;
}
