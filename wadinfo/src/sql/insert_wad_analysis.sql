insert into wad_analysis (wad_id, title, description)
values ($1::uuid, $2::text, $3::text)
on conflict (wad_id) do update
set title = excluded.title,
    description = excluded.description,
    updated_at = now();