-- Search path: case-insensitive substring matches across title, preferred_filename, and wad_filenames.filename.
with candidates as materialized (
  select w.wad_id
  from wads w
  where w.hidden = false
    and w.can_download = true
    and (
      coalesce(w.title, '') ilike ('%' || $1 || '%')
      or coalesce(w.preferred_filename, '') ilike ('%' || $1 || '%')
    )

  union

  select f.wad_id
  from wad_filenames f
  join wads w on w.wad_id = f.wad_id
  where w.hidden = false
    and w.can_download = true
    and f.filename ilike ('%' || $1 || '%')
)
select
  (select count(*) from candidates) as full_count,
  w.wad_id,
  w.meta_json
from candidates c
join wads w on w.wad_id = c.wad_id
order by coalesce(w.title, ''), w.wad_id
offset $2
limit $3;
