-- Clear child rows so re-inserts are idempotent.
-- (Sources are 1:1; we can delete+insert or upsert. We'll upsert sources separately.)
delete from wad_authors        where wad_id = $1::uuid;
delete from wad_descriptions   where wad_id = $1::uuid;
delete from wad_map_list       where wad_id = $1::uuid;
delete from wad_text_files     where wad_id = $1::uuid;

delete from wad_maps           where wad_id = $1::uuid; -- cascades textures/monsters/items via FK, if you used that
delete from wad_counts         where wad_id = $1::uuid;
