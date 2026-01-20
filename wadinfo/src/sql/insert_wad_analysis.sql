insert into wad_analysis (wad_id, title, description, authors)
values ($1::uuid, $2::text, $3::text, $4::text[])
on conflict (wad_id) do update
set title = excluded.title,
    description = excluded.description,
    authors = excluded.authors,
    updated_at = now();