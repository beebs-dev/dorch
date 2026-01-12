with upsert as (
  insert into wads (sha1, filename, wad_type, byte_size, map_count)
  values ($1, $2, $3, $4, $5)
  on conflict (sha1) do update set
    filename  = excluded.filename,
    wad_type  = excluded.wad_type,
    byte_size = excluded.byte_size,
    map_count = excluded.map_count
  returning wad_id
)
select wad_id from upsert
union all
select wad_id from wads where sha1 = $1
limit 1;
