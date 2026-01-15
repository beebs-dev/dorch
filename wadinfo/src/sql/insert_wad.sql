-- Upsert the main WAD row by sha1 and return wad_id.
--
-- NOTE: InsertWadMeta includes an `id: Uuid` field. We store that as wads.wad_id,
-- and also ensure meta_json.id is set to the effective wad_id.
with input as (
  select
    coalesce(
      nullif($1::uuid, '00000000-0000-0000-0000-000000000000'::uuid),
      gen_random_uuid()
    ) as wad_id,
    $2::char(40)     as sha1,
    $3::char(64)     as sha256,
    $4::text         as title,

    $5::text         as file_type,
    $6::bigint       as file_size_bytes,
    $7::text         as file_url,
    coalesce($8::boolean, false) as corrupt,
    $9::text         as corrupt_message,

    $10::text[]      as engines_guess,
    $11::text[]      as iwads_guess,

    $12::timestamptz as wad_archive_updated,
    $13::jsonb       as meta_json
)
insert into wads (
  wad_id,
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
select
  wad_id,
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

  jsonb_set(meta_json, '{id}', to_jsonb(wad_id), true),
  now()
from input
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

  meta_json           = jsonb_set(excluded.meta_json, '{id}', to_jsonb(wads.wad_id), true),
  updated_at          = now()
returning wads.wad_id;
