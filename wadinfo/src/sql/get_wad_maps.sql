select
  wm.map_json || jsonb_build_object(
    'images',
    coalesce(
      jsonb_agg(
        jsonb_build_object(
          'id', wmi.id,
          'url', wmi.url,
          'type', wmi.type
        )
        order by coalesce(wmi.type, ''), wmi.url, wmi.id
      ) filter (where wmi.id is not null),
      '[]'::jsonb
    )
  ) as map_json
from wad_maps wm
left join wad_map_images wmi
  on wmi.wad_id = wm.wad_id
 and wmi.map_name = wm.map_name
where wm.wad_id = $1::uuid
group by wm.map_name, wm.map_json
order by wm.map_name asc;
