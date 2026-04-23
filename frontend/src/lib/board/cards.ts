// Mapping from serde-serialized `Card` enum variants to their displayed
// short labels. Shared by the HUD chip, the discard face decal, and a11y
// announcements so we stay consistent.

export const CARD_DISPLAY: Record<string, string> = {
	One: '1',
	Two: '2',
	Three: '3',
	Four: '4',
	Five: '5',
	Seven: '7',
	Eight: '8',
	Ten: '10',
	Eleven: '11',
	Twelve: '12',
	Sorry: 'Sorry!'
};

export function cardLabel(card: string | null | undefined): string | null {
	if (!card) return null;
	return CARD_DISPLAY[card] ?? card;
}
