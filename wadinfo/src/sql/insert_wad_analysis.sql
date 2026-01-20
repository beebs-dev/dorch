insert into wad_analysis (wad_id, title, release_date, description, authors)
values ($1::uuid, $2::text, $3::text, $4::text, $5::text[])
on conflict (wad_id) do update
set title = excluded.title,
    release_date = excluded.release_date,
    description = excluded.description,
    authors = excluded.authors,
    updated_at = now();