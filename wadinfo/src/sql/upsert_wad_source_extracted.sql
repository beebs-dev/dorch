insert into wad_source_extracted (wad_id, extracted)
values ($1::uuid, $2::jsonb)
on conflict (wad_id) do update set
  extracted = excluded.extracted;
