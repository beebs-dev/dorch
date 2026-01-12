select
  greatest(
    similarity(coalesce(w.title, ''), $1),
    case when w.sha1 = $1 then 1 else 0 end
  ) as rank,
  w.*
from wads w
where coalesce(w.title, '') % $1
   or w.sha1 = $1
order by rank desc, coalesce(w.title, '')
offset $2
limit $3;
