// Minimal rAF-driven tween primitive. Each tween returns a Promise that
// resolves when the animation completes. Callers interpolate their own
// state inside the `onTick` callback — keeps the helper agnostic to
// THREE types and tweenable fields.

export type Easing = (t: number) => number;

export const linear: Easing = (t) => t;
export const easeInOutCubic: Easing = (t) =>
	t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
export const easeOutQuad: Easing = (t) => 1 - (1 - t) * (1 - t);

/**
 * Tween over `durationMs`, invoking `onTick(eased_t)` each frame. If the
 * duration is 0 the callback fires once at t=1 and resolves synchronously
 * (used for prefers-reduced-motion).
 */
export function tween(
	durationMs: number,
	onTick: (t: number) => void,
	easing: Easing = easeInOutCubic
): Promise<void> {
	if (durationMs <= 0) {
		onTick(1);
		return Promise.resolve();
	}
	return new Promise((resolve) => {
		const start = performance.now();
		function step(now: number) {
			const elapsed = now - start;
			const u = Math.min(1, elapsed / durationMs);
			onTick(easing(u));
			if (u < 1) requestAnimationFrame(step);
			else resolve();
		}
		requestAnimationFrame(step);
	});
}

export function wait(ms: number): Promise<void> {
	if (ms <= 0) return Promise.resolve();
	return new Promise((resolve) => setTimeout(resolve, ms));
}
