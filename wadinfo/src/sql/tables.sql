-- init.sql
-- Schema for the merged WAD metadata + per-map stats output.

begin;

-- Extensions
create extension if not exists pgcrypto;
create extension if not exists pg_trgm;

-- ----------------------------
-- Core: one row per WAD (out_obj.meta)
-- ----------------------------
create table if not exists wads (
  wad_id              uuid primary key default gen_random_uuid(),

  -- identity
  sha1                char(40) not null unique,
  sha256              char(64),

  -- merged "best" fields
  title               text,

  -- filenames.json / additional.json
  preferred_filename  text,
  added_at            timestamptz,
  locked              boolean not null default false,
  can_download        boolean not null default true,
  adult               boolean not null default false,
  hidden              boolean not null default false,

  -- file.*
  file_type           text not null,          -- IWAD/PWAD/PK3/UNKNOWN/etc (from wad_archive.type)
  file_size_bytes     bigint,                 -- from wad_archive.size
  file_url            text,                   -- resolved S3 URL
  corrupt             boolean not null default false,
  corrupt_message     text,

  -- content.*
  engines_guess       text[],                 -- from wad_archive.engines (guess)
  iwads_guess         text[],                 -- from wad_archive.iwads   (guess)

  -- sources.wad_archive.updated (string in source store parsed when possible)
  wad_archive_updated timestamptz,

  -- keep full merged meta JSON for forward-compat/debug
  meta_json           jsonb not null,

  created_at          timestamptz not null default now(),
  updated_at          timestamptz not null default now()
);

-- Backfill-friendly: add new columns for existing deployments.
--alter table wads add column if not exists preferred_filename text;
--alter table wads add column if not exists added_at timestamptz;
--alter table wads add column if not exists locked boolean not null default false;
--alter table wads add column if not exists can_download boolean not null default true;
--alter table wads add column if not exists adult boolean not null default false;
--alter table wads add column if not exists hidden boolean not null default false;

create index if not exists idx_wads_title_trgm
  on wads using gin (title gin_trgm_ops);

create index if not exists idx_wads_preferred_filename_trgm
  on wads using gin (preferred_filename gin_trgm_ops);

create index if not exists idx_wads_hidden
  on wads (hidden);

create index if not exists idx_wads_can_download
  on wads (can_download);

-- Common eligibility filter (e.g. featured sampling queries)
create index if not exists idx_wads_hidden_can_download_wad_id
  on wads (hidden, can_download, wad_id);

-- Supports fast featured sampling without `order by random()`.
-- Matches the expressions used in featured_wads_with_images.sql.
create index if not exists idx_wads_featured_title_missing_shuffle
  on wads (
    ((nullif(trim(meta_json->>'title'), '') is null)),
    (md5(wad_id::text))
  )
  where hidden = false and can_download = true;

create index if not exists idx_wads_file_type
  on wads (file_type);

create index if not exists idx_wads_corrupt
  on wads (corrupt);

create index if not exists idx_wads_updated
  on wads (wad_archive_updated);

create index if not exists idx_wads_meta_json
  on wads using gin (meta_json);

-- ----------------------------
-- Multi-valued merged fields: authors / descriptions / map list / text files
-- ----------------------------
create table if not exists wad_filenames (
  wad_id      uuid not null references wads(wad_id) on delete cascade,
  filename    text not null,
  ord         int  not null,
  primary key (wad_id, ord)
);

create index if not exists idx_wad_filenames_trgm
  on wad_filenames using gin (filename gin_trgm_ops);

create index if not exists idx_wad_filenames_filename
  on wad_filenames (filename);

create table if not exists wad_authors (
  wad_id      uuid not null references wads(wad_id) on delete cascade,
  author      text not null,
  ord         int  not null,
  primary key (wad_id, ord)
);

create index if not exists idx_wad_authors_author_trgm
  on wad_authors using gin (author gin_trgm_ops);

create table if not exists wad_descriptions (
  wad_id      uuid not null references wads(wad_id) on delete cascade,
  description text not null,
  ord         int  not null,
  primary key (wad_id, ord)
);

create index if not exists idx_wad_descriptions_trgm
  on wad_descriptions using gin (description gin_trgm_ops);

-- This is the merged/best list of map markers (meta.content.maps), not per-map stats.
create table if not exists wad_map_list (
  wad_id    uuid not null references wads(wad_id) on delete cascade,
  map_name  text not null,   -- MAP01 / E1M1
  ord       int  not null,
  primary key (wad_id, ord)
);

create index if not exists idx_wad_map_list_name
  on wad_map_list (map_name);

-- meta.text_files[] (pk3 embedded text + idgames textfile)
create table if not exists wad_text_files (
  wad_id     uuid not null references wads(wad_id) on delete cascade,
  source     text not null,        -- 'pk3' or 'idgames'
  name       text,                 -- optional filename/path
  contents   text not null,        -- normalized text
  ord        int not null,
  primary key (wad_id, ord)
);

create index if not exists idx_wad_text_files_source
  on wad_text_files (source);

create index if not exists idx_wad_text_files_contents_trgm
  on wad_text_files using gin (contents gin_trgm_ops);

-- ----------------------------
-- Sources snapshots (meta.sources.*)
-- ----------------------------
create table if not exists wad_source_wad_archive (
  wad_id      uuid primary key references wads(wad_id) on delete cascade,
  updated     timestamptz,
  hashes      jsonb                 -- {md5, sha1, sha256} etc
);

create index if not exists idx_wad_source_wad_archive_hashes
  on wad_source_wad_archive using gin (hashes);

create table if not exists wad_source_idgames (
  wad_id      uuid primary key references wads(wad_id) on delete cascade,

  idgames_id  bigint,            -- ig.id (often numeric)
  url         text,
  dir         text,
  filename    text,
  date_text   text,              -- keep as text idgames dates can be inconsistent
  title       text,
  author      text,
  credits     text,
  rating      double precision,
  votes       int,

  raw_json    jsonb
);

create index if not exists idx_wad_source_idgames_title_trgm
  on wad_source_idgames using gin (title gin_trgm_ops);

create index if not exists idx_wad_source_idgames_author_trgm
  on wad_source_idgames using gin (author gin_trgm_ops);

create index if not exists idx_wad_source_idgames_raw_json
  on wad_source_idgames using gin (raw_json);

-- compact extracted snapshot (meta.sources.extracted)
create table if not exists wad_source_extracted (
  wad_id      uuid primary key references wads(wad_id) on delete cascade,
  extracted   jsonb not null
);

create index if not exists idx_wad_source_extracted
  on wad_source_extracted using gin (extracted);

-- ----------------------------
-- WAD Archive counts (meta.content.counts)
-- Choose ONE approach:
--   A) wad_counts (jsonb) and/or
--   B) wad_count_kv (normalized)
-- ----------------------------

-- A) JSONB counts
create table if not exists wad_counts (
  wad_id   uuid primary key references wads(wad_id) on delete cascade,
  counts   jsonb not null
);

create index if not exists idx_wad_counts
  on wad_counts using gin (counts);

-- B) Normalized counts (string->int map)
create table if not exists wad_count_kv (
  wad_id      uuid not null references wads(wad_id) on delete cascade,
  count_key   text not null,
  count_val   int  not null,
  primary key (wad_id, count_key)
);

create index if not exists idx_wad_count_kv_key_val
  on wad_count_kv (count_key, count_val);

-- ----------------------------
-- Per-map stats output (out_obj.maps[])
-- ----------------------------
create table if not exists wad_maps (
  wad_id          uuid not null references wads(wad_id) on delete cascade,
  map_name        text not null,                  -- MAP01 / E1M1

  format          text not null,                  -- doom / hexen / unknown
  compatibility   text not null,                  -- vanilla_or_boom / hexen / unknown

  -- core stats
  things          int not null,
  linedefs        int not null,
  sidedefs        int not null,
  vertices        int not null,
  sectors         int not null,
  segs            int not null,
  ssectors        int not null,
  nodes           int not null,

  -- mechanics
  teleports       boolean not null,
  secret_exit     boolean not null,
  keys            text[] not null default '{}',   -- blue/yellow/red/etc

  -- monsters summary
  monster_total   int not null,
  uv_monsters     int not null,
  hmp_monsters    int not null,
  htr_monsters    int not null,

  -- items summary
  item_total      int not null,
  uv_items        int not null,
  hmp_items       int not null,
  htr_items       int not null,

  -- embedded metadata stub
  title           text,
  music           text,
  metadata_source text not null,                 -- marker (current)
  map_json        jsonb not null,                -- raw map payload for forward-compat/debug

  primary key (wad_id, map_name)
);

create index if not exists idx_wad_maps_map_name
  on wad_maps (map_name);

create index if not exists idx_wad_maps_format
  on wad_maps (format);

create index if not exists idx_wad_maps_compat
  on wad_maps (compatibility);

create index if not exists idx_wad_maps_teleports
  on wad_maps (teleports);

create index if not exists idx_wad_maps_secret_exit
  on wad_maps (secret_exit);

create index if not exists idx_wad_maps_keys_gin
  on wad_maps using gin (keys);

create index if not exists idx_wad_maps_map_json
  on wad_maps using gin (map_json);

-- Textures list (maps[].stats.textures[])
create table if not exists wad_map_textures (
  wad_id    uuid not null references wads(wad_id) on delete cascade,
  map_name  text not null,
  texture   text not null,
  count     int  not null default 1,
  primary key (wad_id, map_name, texture),
  foreign key (wad_id, map_name) references wad_maps(wad_id, map_name) on delete cascade
);


create index if not exists idx_wad_map_textures_texture
  on wad_map_textures (texture);

-- Monster breakdown (maps[].monsters.by_type)
create table if not exists wad_map_monsters (
  wad_id    uuid not null references wads(wad_id) on delete cascade,
  map_name  text not null,
  monster   text not null,      -- zombieman/imp/etc
  count     int  not null,
  primary key (wad_id, map_name, monster),
  foreign key (wad_id, map_name) references wad_maps(wad_id, map_name) on delete cascade
);

create index if not exists idx_wad_map_monsters_monster
  on wad_map_monsters (monster);

create index if not exists idx_wad_map_monsters_count
  on wad_map_monsters (count);

-- Item breakdown (maps[].items.by_type)
create table if not exists wad_map_items (
  wad_id    uuid not null references wads(wad_id) on delete cascade,
  map_name  text not null,
  item      text not null,      -- shotgun/medikit/etc
  count     int  not null,
  primary key (wad_id, map_name, item),
  foreign key (wad_id, map_name) references wad_maps(wad_id, map_name) on delete cascade
);

create index if not exists idx_wad_map_items_item
  on wad_map_items (item);

create index if not exists idx_wad_map_items_count
  on wad_map_items (count);

-- ----------------------------
-- Per-map images (screenshots, panoramas, etc)
-- ----------------------------
create table if not exists wad_map_images (
  id        uuid primary key default gen_random_uuid(),
  wad_id    uuid not null references wads(wad_id) on delete cascade,
  map_name  text not null,
  url       text not null,
  type      text
);

create index if not exists idx_wad_map_images_wad_map
  on wad_map_images (wad_id, map_name);

-- Helps `jsonb_agg(... order by i.map_name, i.id)` by providing a matching
-- ordered access path per wad.
create index if not exists idx_wad_map_images_wad_map_id
  on wad_map_images (wad_id, map_name, id);

create table if not exists wad_analysis (
  wad_id      uuid primary key references wads(wad_id) on delete cascade,
  title       text,
  release_date text,
  description text not null,
  authors     text[] not null default '{}',
  created_at  timestamptz not null default now(),
  updated_at  timestamptz not null default now(),
  origin      text
);

create table if not exists wad_analysis_tags (
  wad_id      uuid references wads(wad_id) on delete cascade,
  tag         text not null,
  primary key (wad_id, tag)
);

create table if not exists wad_map_analysis (
  wad_id      uuid not null references wads(wad_id) on delete cascade,
  map_name    text not null,
  map_title   text,
  description text not null,
  authors     text[] not null default '{}',
  created_at  timestamptz not null default now(),
  updated_at  timestamptz not null default now(),
  origin      text,
  primary key (wad_id, map_name)
);

create table if not exists wad_map_analysis_tags (
  wad_id      uuid references wads(wad_id) on delete cascade,
  map_name    text not null,
  tag         text not null,
  primary key (wad_id, map_name, tag)
);