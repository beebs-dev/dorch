with candidates as materialized (
  select w.wad_id
  from wads w
  where coalesce(w.title, '') % $1

  union
  select a.wad_id
  from wad_authors a
  where a.author % $1

  union
  select d.wad_id
  from wad_descriptions d
  where d.description % $1

  union
  select w.wad_id
  from wads w
  where w.sha1 = $1
),
ranked as (
  select
    w.wad_id,
    w.meta_json,
    greatest(
      similarity(coalesce(w.title, ''), $1),
      coalesce((
        select max(similarity(a.author, $1))
        from wad_authors a
        where a.wad_id = w.wad_id
      ), 0),
      coalesce((
        select max(similarity(d.description, $1))
        from wad_descriptions d
        where d.wad_id = w.wad_id
      ), 0),
      case when w.sha1 = $1 then 1 else 0 end
    ) as rank
  from wads w
  join candidates c on c.wad_id = w.wad_id
)
select
  (select count(*) from candidates) as full_count,
  r.wad_id,
  r.meta_json,
  r.rank
from ranked r
order by r.rank desc, coalesce(r.meta_json->>'title', '')
offset $2
limit $3;
