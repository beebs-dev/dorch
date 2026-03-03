import { DOOM_PALETTE_COLORS } from '$lib/utils/colors';

export const LS_PLAYER_COLOR_KEY = 'dorch.settings.player_color';

export type NamedPlayerColor = {
    key: string;
    label: string;
    rgb: [number, number, number];
    paletteIndex: number;
};

type NamedColorDef = { label: string; rgb: [number, number, number] };

const NAMED_PLAYER_COLOR_MAP: ReadonlyMap<string, NamedColorDef> = new Map([
    ['grey1', { label: 'Grey 1', rgb: [85, 85, 85] }],
    ['grey2', { label: 'Grey 2', rgb: [171, 171, 171] }],
    ['grey3', { label: 'Grey 3', rgb: [50, 50, 50] }],
    ['grey4', { label: 'Grey 4', rgb: [210, 210, 210] }],
    ['grey5', { label: 'Grey 5', rgb: [128, 128, 128] }],
    ['grey6', { label: 'Grey 6', rgb: [139, 139, 139] }],
    ['red', { label: 'Red', rgb: [255, 0, 0] }],
    ['red1', { label: 'Red 1', rgb: [255, 127, 127] }],
    ['red2', { label: 'Red 2', rgb: [227, 0, 0] }],
    ['red3', { label: 'Red 3', rgb: [255, 31, 31] }],
    ['red4', { label: 'Red 4', rgb: [203, 0, 0] }],
    ['green', { label: 'Green', rgb: [0, 200, 0] }],
    ['green1', { label: 'Green 1', rgb: [127, 255, 127] }],
    ['green2', { label: 'Green 2', rgb: [71, 131, 58] }],
    ['blue', { label: 'Blue', rgb: [0, 0, 255] }],
    ['blue1', { label: 'Blue 1', rgb: [127, 127, 255] }],
    ['yellow', { label: 'Yellow', rgb: [255, 255, 0] }],
    ['yellow1', { label: 'Yellow 1', rgb: [255, 255, 180] }],
    ['yellow2', { label: 'Yellow 2', rgb: [255, 255, 35] }],
    ['yellow3', { label: 'Yellow 3', rgb: [255, 255, 71] }],
    ['black', { label: 'Black', rgb: [0, 0, 0] }],
    ['purple', { label: 'Purple', rgb: [120, 0, 160] }],
    ['purple1', { label: 'Purple 1', rgb: [200, 30, 255] }],
    ['purple2', { label: 'Purple 2', rgb: [207, 0, 207] }],
    ['purple3', { label: 'Purple 3', rgb: [255, 0, 255] }],
    ['white', { label: 'White', rgb: [255, 255, 255] }],
    ['rblue1', { label: 'Royal Blue 1', rgb: [81, 81, 255] }],
    ['rblue2', { label: 'Royal Blue 2', rgb: [0, 0, 227] }],
    ['rblue3', { label: 'Royal Blue 3', rgb: [0, 0, 130] }],
    ['rblue4', { label: 'Royal Blue 4', rgb: [0, 0, 80] }],
    ['orange', { label: 'Orange', rgb: [255, 120, 0] }],
    ['yorange', { label: 'Yellow Orange', rgb: [255, 170, 0] }],
    ['dred', { label: 'Dark Red', rgb: [91, 3, 3] }],
    ['dred2', { label: 'Dark Red 2', rgb: [127, 3, 3] }],
    ['dred3', { label: 'Dark Red 3', rgb: [227, 0, 0] }],
    ['dred4', { label: 'Dark Red 4', rgb: [255, 31, 31] }],
    ['maroon1', { label: 'Maroon 1', rgb: [154, 49, 49] }],
    ['maroon2', { label: 'Maroon 2', rgb: [125, 24, 24] }],
    ['gold1', { label: 'Gold 1', rgb: [204, 168, 62] }],
    ['gold2', { label: 'Gold 2', rgb: [186, 139, 44] }],
    ['cyan1', { label: 'Cyan 1', rgb: [0, 255, 255] }],
    ['cyan2', { label: 'Cyan 2', rgb: [81, 255, 255] }]
]);

function rgbKey(rgb: [number, number, number]): string {
    return `${rgb[0]},${rgb[1]},${rgb[2]}`;
}

const PALETTE_INDEX_BY_RGB = new Map<string, number>();
for (let index = 0; index < DOOM_PALETTE_COLORS.length; index++) {
    const rgb = DOOM_PALETTE_COLORS[index] as [number, number, number];
    const key = rgbKey(rgb);
    if (!PALETTE_INDEX_BY_RGB.has(key)) {
        PALETTE_INDEX_BY_RGB.set(key, index);
    }
}

function squaredDistance(a: [number, number, number], b: [number, number, number]): number {
    const dr = a[0] - b[0];
    const dg = a[1] - b[1];
    const db = a[2] - b[2];
    return dr * dr + dg * dg + db * db;
}

function resolvePaletteIndex(rgb: [number, number, number]): number {
    const exact = PALETTE_INDEX_BY_RGB.get(rgbKey(rgb));
    if (exact !== undefined) return exact;

    let bestIndex = 0;
    let bestDistance = Number.POSITIVE_INFINITY;
    for (let index = 0; index < DOOM_PALETTE_COLORS.length; index++) {
        const candidate = DOOM_PALETTE_COLORS[index] as [number, number, number];
        const distance = squaredDistance(rgb, candidate);
        if (distance < bestDistance) {
            bestDistance = distance;
            bestIndex = index;
        }
    }
    return bestIndex;
}

export const NAMED_PLAYER_COLORS: NamedPlayerColor[] = Array.from(NAMED_PLAYER_COLOR_MAP.entries()).map(
    ([key, value]) => ({
        key,
        label: value.label,
        rgb: value.rgb,
        paletteIndex: resolvePaletteIndex(value.rgb)
    })
);

export const NAMED_PLAYER_COLOR_INDEX_SET = new Set<number>(
    NAMED_PLAYER_COLORS.map((item) => item.paletteIndex)
);

export function clampPlayerColorIndex(value: number): number {
    if (!Number.isFinite(value)) return 0;
    if (value < 0) return 0;
    if (value > 255) return 255;
    return Math.trunc(value);
}

export function parsePlayerColorIndex(value: string | number | null | undefined): number | null {
    if (value === null || value === undefined) return null;
    if (typeof value === 'number') return clampPlayerColorIndex(value);
    const trimmed = value.trim();
    if (!trimmed) return null;
    if (!/^\d+$/.test(trimmed)) return null;
    return clampPlayerColorIndex(Number.parseInt(trimmed, 10));
}

export function toPlayerColorHex(index: number): string {
    const paletteIndex = clampPlayerColorIndex(index);
    const rgb = DOOM_PALETTE_COLORS[paletteIndex] as [number, number, number];
    const hex = rgb
        .map((channel) => Math.max(0, Math.min(255, channel)).toString(16).padStart(2, '0'))
        .join('');
    return `#${hex}`;
}
