insert into wad_source_idgames (
  wad_id,
  idgames_id, url, dir, filename, date_text,
  title, author, credits, rating, votes,
  raw_json
)
values (
  $1::uuid,
  $2::bigint, $3::text, $4::text, $5::text, $6::text,
  $7::text, $8::text, $9::text, $10::double precision, $11::int,
  $12::jsonb
)
on conflict (wad_id) do update set
  idgames_id = excluded.idgames_id,
  url        = excluded.url,
  dir        = excluded.dir,
  filename   = excluded.filename,
  date_text  = excluded.date_text,
  title      = excluded.title,
  author     = excluded.author,
  credits    = excluded.credits,
  rating     = excluded.rating,
  votes      = excluded.votes,
  raw_json   = excluded.raw_json;
