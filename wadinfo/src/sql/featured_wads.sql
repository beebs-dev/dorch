select
  count(*) over() as full_count,
  w.wad_id,
  w.meta_json
from wads w
where w.hidden = false
  and w.can_download = true
  and exists (select 1 from wad_map_images i where i.wad_id = w.wad_id)
order by
  (nullif(trim(w.meta_json->>'title'), '') is null) asc,
  random()
limit $1;
