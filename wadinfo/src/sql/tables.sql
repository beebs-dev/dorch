create extension if not exists pg_trgm;

create table if not exists wads (
  wad_id          uuid primary key default gen_random_uuid(),
  sha1            text not null unique,
  filename        text,
  wad_type        text check (wad_type in ('IWAD','PWAD')),
  map_count       int not null,

  -- optional: storage
  byte_size       bigint,
  uploaded_at     timestamptz default now()
);
create index if not exists wads_filename_trgm_idx
on wads
using gin (filename gin_trgm_ops);

create table if not exists wad_maps (
  wad_id          uuid not null references wads(wad_id) on delete cascade,
  map_name        text not null,                  -- MAP01 / E1M1
  format          text not null,                  -- doom / hexen / unknown
  compatibility   text not null,                  -- vanilla_or_boom / hexen / unknown

  -- core stats (fast filters)
  things          int not null,
  linedefs        int not null,
  sidedefs        int not null,
  vertices        int not null,
  sectors         int not null,
  segs            int not null,
  ssectors        int not null,
  nodes           int not null,

  -- computed flags (fast filters)
  teleports       boolean not null,
  secret_exit     boolean not null,

  -- monsters summary
  monster_total   int not null,
  uv_monsters     int not null,
  hmp_monsters    int not null,
  htr_monsters    int not null,

  -- per-monster breakdown (counts, 0 if none)
  zombieman_count        int not null default 0,
  shotgun_guy_count      int not null default 0,
  chaingun_guy_count     int not null default 0,
  imp_count              int not null default 0,
  demon_count            int not null default 0,
  spectre_count          int not null default 0,
  cacodemon_count        int not null default 0,
  lost_soul_count        int not null default 0,
  pain_elemental_count   int not null default 0,
  revenant_count         int not null default 0,
  mancubus_count         int not null default 0,
  arachnotron_count      int not null default 0,
  hell_knight_count      int not null default 0,
  baron_count            int not null default 0,
  archvile_count         int not null default 0,
  cyberdemon_count       int not null default 0,
  spider_mastermind_count int not null default 0,

  -- keys: array of key types present (e.g. {'red','blue_skull'})
  keys            text[] not null default '{}',

  -- keep everything for future-proofing
  doc             jsonb not null,

  primary key (wad_id, map_name),

  -- sanity check (optional): total should be >= sum of known monster columns
  check (
    monster_total >= (
      zombieman_count + shotgun_guy_count + chaingun_guy_count +
      imp_count + demon_count + spectre_count +
      cacodemon_count + lost_soul_count + pain_elemental_count +
      revenant_count + mancubus_count + arachnotron_count +
      hell_knight_count + baron_count + archvile_count +
      cyberdemon_count + spider_mastermind_count
    )
  )
);

create index if not exists wad_maps_map_name_idx on wad_maps (map_name);
create index if not exists wad_maps_compat_idx on wad_maps (compatibility);
create index if not exists wad_maps_format_idx on wad_maps (format);
create index if not exists wad_maps_monster_total_idx on wad_maps (monster_total);
create index if not exists wad_maps_sectors_idx on wad_maps (sectors);
create index if not exists wad_maps_linedefs_idx on wad_maps (linedefs);
create index if not exists wad_maps_teleports_true_idx on wad_maps (wad_id, map_name) where teleports = true;
create index if not exists wad_maps_secret_exit_true_idx on wad_maps (wad_id, map_name) where secret_exit = true;
create index if not exists wad_maps_keys_gin on wad_maps using gin (keys);