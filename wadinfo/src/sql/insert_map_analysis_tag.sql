insert into wad_map_analysis_tags (wad_id, map_name, tag)
values ($1::uuid, $2::text, $3::text)
on conflict do nothing;