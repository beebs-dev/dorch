insert into wad_counts (wad_id, counts)
values ($1::uuid, $2::jsonb)
on conflict (wad_id) do update set
  counts = excluded.counts;
