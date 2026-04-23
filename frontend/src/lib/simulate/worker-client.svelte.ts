import type { GameHistory } from '$lib/board/actions';
import type {
	GameSummary,
	ProgressStats,
	SimConfig,
	WorkerRequest,
	WorkerResponse
} from './types';

/** Reactive wrapper around the simulation worker. */
export class SimulatorClient {
	#worker: Worker | null = null;
	#ready = $state(false);
	#running = $state(false);
	#paused = $state(false);
	#stats = $state<ProgressStats | null>(null);
	#games = $state<GameSummary[]>([]);
	#histories = $state<GameHistory[] | null>(null);
	#error = $state<string | null>(null);
	#finished = $state(false);

	get ready() {
		return this.#ready;
	}
	get running() {
		return this.#running;
	}
	get paused() {
		return this.#paused;
	}
	get stats() {
		return this.#stats;
	}
	get games() {
		return this.#games;
	}
	get histories() {
		return this.#histories;
	}
	get error() {
		return this.#error;
	}
	get finished() {
		return this.#finished;
	}

	ensureWorker() {
		if (this.#worker) return;
		this.#worker = new Worker(new URL('../worker.ts', import.meta.url), {
			type: 'module'
		});
		this.#worker.onmessage = (e: MessageEvent<WorkerResponse>) => this.#onMessage(e.data);
		this.#worker.onerror = (e) => {
			this.#error = e.message || 'Worker error';
			this.#running = false;
		};
	}

	#send(req: WorkerRequest) {
		this.ensureWorker();
		this.#worker?.postMessage(req);
	}

	#onMessage(msg: WorkerResponse) {
		switch (msg.type) {
			case 'ready':
				this.#ready = true;
				break;
			case 'progress':
				this.#stats = msg.stats;
				if (msg.new_games.length > 0) {
					this.#games = [...this.#games, ...msg.new_games];
				}
				break;
			case 'complete':
				this.#stats = msg.stats;
				if (msg.histories) this.#histories = msg.histories;
				this.#running = false;
				this.#finished = true;
				break;
			case 'stopped':
				this.#stats = msg.stats;
				this.#running = false;
				break;
			case 'error':
				this.#error = msg.message;
				this.#running = false;
				break;
		}
	}

	start(config: SimConfig) {
		this.ensureWorker();
		this.#stats = null;
		this.#games = [];
		this.#histories = null;
		this.#error = null;
		this.#running = true;
		this.#paused = false;
		this.#finished = false;
		this.#send({ type: 'start', config });
	}

	pause() {
		if (!this.#running || this.#paused) return;
		this.#paused = true;
		this.#send({ type: 'pause' });
	}

	resume() {
		if (!this.#running || !this.#paused) return;
		this.#paused = false;
		this.#send({ type: 'resume' });
	}

	stop() {
		if (!this.#running) return;
		this.#send({ type: 'stop' });
	}

	dispose() {
		this.#worker?.terminate();
		this.#worker = null;
	}
}
