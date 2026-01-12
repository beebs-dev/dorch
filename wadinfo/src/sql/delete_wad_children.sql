-- Clear child rows so re-inserts are idempotent.
-- (Sources are 1:1; we can delete+insert or upsert. We'll upsert sources separately.)
with
	del_authors as (
		delete from wad_authors where wad_id = $1::uuid returning 1
	),
	del_descriptions as (
		delete from wad_descriptions where wad_id = $1::uuid returning 1
	),
	del_map_list as (
		delete from wad_map_list where wad_id = $1::uuid returning 1
	),
	del_text_files as (
		delete from wad_text_files where wad_id = $1::uuid returning 1
	),
	del_maps as (
		delete from wad_maps where wad_id = $1::uuid returning 1
	)
delete from wad_counts where wad_id = $1::uuid;
