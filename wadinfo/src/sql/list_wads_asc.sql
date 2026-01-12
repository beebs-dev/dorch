select
  count(*) over() as full_count,
  w.meta_json
from wads w
order by lower(coalesce(w.title, '')) asc, w.wad_id asc
offset $1
limit $2;
