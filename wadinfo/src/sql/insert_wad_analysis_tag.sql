insert into wad_analysis_tags (wad_id, tag)
values ($1::uuid, $2::text)
on conflict do nothing;