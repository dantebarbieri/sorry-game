import { PLAYER_COLORS_CLASSIC, PLAYER_COLORS_MODERN } from './theme';

export interface BoardSkinPalette {
	/** Base board surface color. */
	board: string;
	/** Default track tile color (before per-tile highlights). */
	trackTile: string;
	/** Scene backdrop. */
	background: string;
	/** One color per `PlayerId` slot — slide, safety channel, home, start. */
	players: readonly string[];
	/**
	 * How slide-direction markers (triangle + end disk) derive their color
	 * from the owner's player color. `darker` tints toward black (reads as
	 * an inked stamp on bright slides over a light board); `lighter` tints
	 * toward white (reads as a painted highlight over a dark board).
	 */
	markerShade: 'darker' | 'lighter';
}

export interface BoardSkin {
	id: string;
	palette: BoardSkinPalette;
}

/** Light-mode board — cream surface, primary player colors, inked markers. */
export const LIGHT_SKIN: BoardSkin = {
	id: 'light',
	palette: {
		board: '#EFE6D2',
		trackTile: '#FBF6E7',
		background: '#1a1d24',
		players: PLAYER_COLORS_CLASSIC,
		markerShade: 'darker'
	}
};

/** Dark-mode board — navy surface, slightly brighter player colors, highlighted markers. */
export const DARK_SKIN: BoardSkin = {
	id: 'dark',
	palette: {
		board: '#2A3B52',
		trackTile: '#D8DEE9',
		background: '#0E141C',
		players: PLAYER_COLORS_MODERN,
		markerShade: 'lighter'
	}
};

export const ALL_SKINS: readonly BoardSkin[] = [LIGHT_SKIN, DARK_SKIN];
