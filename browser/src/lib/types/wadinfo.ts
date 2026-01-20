export type Uuid = string;

export interface WadFileMeta {
	/** "PWAD" | "IWAD" | "PK3" | ... */
	type?: string;
	size?: number | null;
	url?: string | null;
	corrupt?: boolean | null;
	corruptMessage?: string | null;
}

export interface WadContentMeta {
	maps?: string[] | null;
	counts?: Record<string, number> | null;
	engines_guess?: string[] | null;
	iwads_guess?: string[] | null;
}

export interface WadAnalysisMeta {
	title?: string | null;
	description?: string | null;
	/** Optional author list inferred by analysis, used as a fallback when meta.authors is missing. */
	authors?: string[] | null;
	tags?: string[] | null;
}

export interface WadTextFile {
	/** Where this text file came from (e.g. "pk3" | "idgames"). */
	source: string;
	/** Optional filename/path. */
	name?: string | null;
	/** Normalized text contents. */
	contents: string;
}

export interface WadMeta {
	id: Uuid;
	sha1: string;
	sha256?: string | null;
	title?: string | null;
	authors?: string[] | null;
	file: WadFileMeta;
	filenames?: string[] | null;
	filename?: string | null;
	content: WadContentMeta;
	analysis?: WadAnalysisMeta | null;
	/** Optional, may be omitted by some producers. */
	text_files?: WadTextFile[] | null;
}

export interface MapStats {
	things?: number;
	linedefs?: number;
	sidedefs?: number;
	vertices?: number;
	sectors?: number;
	segs?: number;
	ssectors?: number;
	nodes?: number;
	/**
	 * Per-map texture usage.
	 * New shape: { "TEXNAME": useCount }.
	 * Legacy shape (older producers): string[] of texture names.
	 */
	textures?: Record<string, number> | string[];
}

export interface MapMonsters {
	total?: number;
	by_type?: Record<string, number>;
}

export interface MapItems {
	total?: number;
	by_type?: Record<string, number>;
}

export interface MapMechanics {
	teleports?: boolean;
	keys?: string[];
	secret_exit?: boolean;
}

export interface MapDifficulty {
	uv_monsters?: number;
	hmp_monsters?: number;
	htr_monsters?: number;
	uv_items?: number;
	hmp_items?: number;
	htr_items?: number;
}

export interface MapMetadata {
	title?: string | null;
	music?: string | null;
	source?: string;
	// The map detail endpoint includes this under metadata.
	wad_meta?: WadMeta;
}

export interface MapStat {
	map: string;
	format?: string;
	stats?: MapStats;
	monsters?: MapMonsters;
	items?: MapItems;
	mechanics?: MapMechanics;
	difficulty?: MapDifficulty;
	compatibility?: string;
	metadata?: MapMetadata;
	analysis?: WadAnalysisMeta | null;

	/** Per-map screenshot/panorama metadata (embedded by read endpoints). */
	images?: WadImage[];
}

export interface ListResponse<TItem> {
	items: TItem[];
	full_count: number;
	offset: number;
	limit: number;
	truncated: boolean;
}

export type ListWadsResponse = ListResponse<WadMeta>;

export interface GetWadResponse {
	meta: WadMeta;
	maps: MapStat[];
}

export type GetWadMapResponse = MapStat & { wad_meta: WadMeta };

export interface WadSearchResults extends ListResponse<WadMeta> {
	request_id: string;
	query: string;
}

export interface WadImage {
	id?: string | null;
	url: string;
	/** "screenshot" | "panorama" | ... (server uses field name `type`) */
	type?: string | null;
	/** Some producers may emit `kind` instead of `type`. */
	kind?: string | null;
}

export interface MapReference {
	wad_id: Uuid;
	map: string;
}

export interface MapThumbnail {
	wad_id: Uuid;
	map: string;
	url: string;
}

export interface ResolveMapThumbnailsRequest {
	items: MapReference[];
}

export interface ResolveMapThumbnailsResponse {
	items: MapThumbnail[];
}

// Future wiring: ratings are not in the schema yet.
export interface RatingSummary {
	average?: number | null; // 1..5
	count?: number | null;
}
