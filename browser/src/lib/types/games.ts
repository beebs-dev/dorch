export type Uuid = string;

export type WadReference = {
	name: string;
	id: Uuid;
};

export type GameInfo = {
	game_id: Uuid;
	private: boolean;
	name: string;
	max_players: number;
	player_count: number;
	skill: number;
	current_map: string;
	server_started_at?: number | null;
	map_started_at?: number | null;
	monster_kill_count: number;
	monster_count: number;
	motd?: string | null;
	sv_cheats: boolean;
	sv_allowchat: boolean;
	sv_allowvoicechat: boolean;
	sv_fastmonsters: boolean;
	sv_monsters: boolean;
	sv_nomonsters: boolean;
	sv_itemsrespawn: boolean;
	sv_itemrespawntime?: number | null;
	sv_coop_damagefactor?: number | null;
	sv_nojump: boolean;
	sv_nocrouch: boolean;
	sv_nofreelook: boolean;
	sv_respawnonexit: boolean;
	sv_timelimit?: number | null;
	sv_fraglimit?: number | null;
	sv_scorelimit?: number | null;
	sv_duellimit?: number | null;
	sv_roundlimit?: number | null;
	sv_allowrun: boolean;
	sv_allowfreelook: boolean;
};

export type GameSummary = {
	game_id: Uuid;
	iwad: WadReference;
	files?: WadReference[] | null;
	info?: GameInfo | null;
};

export type ListGamesResponse = {
	games: GameSummary[];
};
