// Describe engine actions as English sentences for the ARIA live region
// + any future transcript UI. Kept DOM-free so it can be unit-tested.

import type { BoardGeometry, PlayerId, SpaceId } from './geometry';
import type { BumpEvent, Move, PlayRecord, SlideEvent } from './actions';
import { cardLabel } from './cards';

const DEFAULT_PLAYER_NAMES = ['Red', 'Blue', 'Yellow', 'Green'];

export function playerName(player: PlayerId, names = DEFAULT_PLAYER_NAMES): string {
	return names[player] ?? `Player ${player}`;
}

export function describeSpace(
	geometry: BoardGeometry,
	id: SpaceId,
	names = DEFAULT_PLAYER_NAMES
): string {
	const layout = geometry.spaces.find((s) => s.id === id);
	if (!layout) return `space ${id}`;
	const kind = layout.kind;
	if ('Track' in kind) return `track ${kind.Track}`;
	if ('StartArea' in kind) return `${playerName(kind.StartArea, names)} start`;
	if ('Home' in kind) return `${playerName(kind.Home, names)} home`;
	if ('Safety' in kind) return `${playerName(kind.Safety[0], names)} safety ${kind.Safety[1] + 1}`;
	return `space ${id}`;
}

function describeMove(
	mv: Move,
	player: PlayerId,
	geometry: BoardGeometry,
	names: string[]
): string {
	const name = playerName(player, names);
	switch (mv.type) {
		case 'Advance':
			return `${name} pawn ${mv.pawn} advanced ${mv.card_value} spaces to ${describeSpace(geometry, mv.to, names)}.`;
		case 'Retreat':
			return `${name} pawn ${mv.pawn} moved back ${mv.card_value} to ${describeSpace(geometry, mv.to, names)}.`;
		case 'StartPawn':
			return `${name} pawn ${mv.pawn} left Start for ${describeSpace(geometry, mv.to, names)}.`;
		case 'Sorry':
			return `${name} pawn ${mv.my_pawn} used Sorry! to bump ${playerName(mv.their_player, names)} pawn ${mv.their_pawn} back to Start and landed at ${describeSpace(geometry, mv.to, names)}.`;
		case 'SwapEleven':
			return `${name} pawn ${mv.my_pawn} swapped places with ${playerName(mv.their_player, names)} pawn ${mv.their_pawn}.`;
		case 'SplitSeven':
			return `${name} split a seven — pawn ${mv.first.pawn} moved ${mv.first.steps} to ${describeSpace(geometry, mv.first.to, names)}, pawn ${mv.second.pawn} moved ${mv.second.steps} to ${describeSpace(geometry, mv.second.to, names)}.`;
		case 'Pass':
			return `${name} had no legal move and passed.`;
	}
}

function describeBumps(
	bumps: BumpEvent[],
	exclude: Set<string>,
	names: string[]
): string {
	const items = bumps
		.filter((b) => !exclude.has(`${b.player}:${b.pawn}:${b.from}`))
		.map((b) => `${playerName(b.player, names)} pawn ${b.pawn}`);
	if (items.length === 0) return '';
	if (items.length === 1) return ` Bumped ${items[0]} back to Start.`;
	return ` Bumped ${items.join(', ')} back to Start.`;
}

function describeSlides(
	slides: SlideEvent[],
	geometry: BoardGeometry,
	names: string[]
): { sentence: string; bumpsHandled: Set<string> } {
	const handled = new Set<string>();
	if (slides.length === 0) return { sentence: '', bumpsHandled: handled };
	const parts = slides.map((s) => {
		const owner = playerName(s.player, names);
		return `${owner} pawn ${s.pawn} slid to ${describeSpace(geometry, s.to, names)}`;
	});
	return { sentence: ` ${parts.join('. ')}.`, bumpsHandled: handled };
}

/**
 * One-sentence natural-language description of a `Play` action suitable
 * for an ARIA live region. Composes move + slides + bumps in the order
 * they happen visually.
 */
export function describeAction(
	play: PlayRecord,
	player: PlayerId,
	geometry: BoardGeometry,
	names: string[] = DEFAULT_PLAYER_NAMES
): string {
	const card = cardLabel(play.card) ?? play.card;
	const header = `${playerName(player, names)} played ${card}. `;
	const move = describeMove(play.mv, player, geometry, names);
	const slides = describeSlides(play.slides, geometry, names);
	const bumps = describeBumps(play.bumps, slides.bumpsHandled, names);
	return `${header}${move}${slides.sentence}${bumps}`;
}
