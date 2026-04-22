// Default player colors. Skins override these; this module exports the
// canonical Sorry! primary palette.

export const PLAYER_COLORS_CLASSIC: readonly string[] = [
	'#E02828', // P0 red
	'#2B6CD4', // P1 blue
	'#E5B712', // P2 yellow
	'#2E9F4A' // P3 green
];

/** Slightly brighter variants, readable against a dark backdrop. */
export const PLAYER_COLORS_MODERN: readonly string[] = [
	'#F16161',
	'#5FA8FF',
	'#FCD34D',
	'#6EE7A2'
];
