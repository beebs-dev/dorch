select
  count(*) over() as full_count,
  w.wad_id,
  w.meta_json
from wads w
where w.hidden = false and w.can_download = true
order by
  w.has_images desc,
  lower(coalesce(w.title, '')) asc,
  w.wad_id asc
offset $1
limit $2;
