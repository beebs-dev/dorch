insert into wad_source_wad_archive (wad_id, updated, hashes)
values ($1::uuid, $2::timestamptz, $3::jsonb)
on conflict (wad_id) do update set
  updated = excluded.updated,
  hashes  = excluded.hashes;
