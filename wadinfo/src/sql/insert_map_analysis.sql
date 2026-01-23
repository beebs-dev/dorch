insert into wad_map_analysis (wad_id, map_name, map_title, description, authors, origin)
values ($1::uuid, $2::text, $3::text, $4::text, $5::text[], $6::text)
on conflict (wad_id, map_name) do update
set map_title = excluded.map_title,
    description = excluded.description,
    authors = excluded.authors,
    updated_at = now(),
    origin = excluded.origin;