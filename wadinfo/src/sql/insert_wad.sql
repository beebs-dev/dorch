-- Upsert the main WAD row by sha1 and return wad_id.
insert into wads (
  sha1,
  sha256,
  title,

  file_type,
  file_size_bytes,
  file_url,
  corrupt,
  corrupt_message,

  engines_guess,
  iwads_guess,

  wad_archive_updated,

  meta_json,
  updated_at
)
values (
  $1::char(40),
  $2::char(64),
  $3::text,

  $4::text,
  $5::bigint,
  $6::text,
  coalesce($7::boolean, false),
  $8::text,

  $9::text[],
  $10::text[],

  $11::timestamptz,

  $12::jsonb,
  now()
)
on conflict (sha1) do update set
  sha256              = excluded.sha256,
  title               = excluded.title,

  file_type           = excluded.file_type,
  file_size_bytes     = excluded.file_size_bytes,
  file_url            = excluded.file_url,
  corrupt             = excluded.corrupt,
  corrupt_message     = excluded.corrupt_message,

  engines_guess       = excluded.engines_guess,
  iwads_guess         = excluded.iwads_guess,

  wad_archive_updated = excluded.wad_archive_updated,

  meta_json           = excluded.meta_json,
  updated_at          = now()
returning wad_id;
