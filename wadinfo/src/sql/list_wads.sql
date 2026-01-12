select
	count(*) over() as full_count,
	w.*
from wads w
order by w.wad_archive_updated desc nulls last, w.updated_at desc
offset $1
limit $2;