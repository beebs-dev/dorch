-- Clear child rows so re-inserts are idempotent.
--
-- IMPORTANT: data-modifying CTEs must be referenced, otherwise Postgres may
-- treat them as unused and skip executing them. We reference every CTE via
-- UNION ALL below to force execution.
with
	-- Delete per-map breakdown tables explicitly (even though FK cascades should
	-- cover them) so this remains safe across older schemas.
	del_map_textures as (
		delete from wad_map_textures where wad_id = $1::uuid returning 1
	),
	del_map_monsters as (
		delete from wad_map_monsters where wad_id = $1::uuid returning 1
	),
	del_map_items as (
		delete from wad_map_items where wad_id = $1::uuid returning 1
	),

	del_maps as (
		delete from wad_maps where wad_id = $1::uuid returning 1
	),
	del_authors as (
		delete from wad_authors where wad_id = $1::uuid returning 1
	),
	del_descriptions as (
		delete from wad_descriptions where wad_id = $1::uuid returning 1
	),
	del_filenames as (
		delete from wad_filenames where wad_id = $1::uuid returning 1
	),
	del_map_list as (
		delete from wad_map_list where wad_id = $1::uuid returning 1
	),
	del_text_files as (
		delete from wad_text_files where wad_id = $1::uuid returning 1
	),
	del_counts as (
		delete from wad_counts where wad_id = $1::uuid returning 1
	),
	del_count_kv as (
		delete from wad_count_kv where wad_id = $1::uuid returning 1
	)
select 1
from (
	select 1 from del_map_textures
	union all select 1 from del_map_monsters
	union all select 1 from del_map_items
	union all select 1 from del_maps
	union all select 1 from del_authors
	union all select 1 from del_descriptions
	union all select 1 from del_filenames
	union all select 1 from del_map_list
	union all select 1 from del_text_files
	union all select 1 from del_counts
	union all select 1 from del_count_kv
) _
limit 1;
