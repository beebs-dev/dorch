select
  count(*) over() as full_count,
  w.wad_id,
  w.meta_json
from wads w
order by lower(coalesce(w.title, '')) desc, w.wad_id desc
offset $1
limit $2;
