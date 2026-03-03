update user_profile
set
  display_name = $2,
  avatar_url = $3,
  player_color = $4,
  last_active_at = $5,
  privacy_hide_activity = $6
where id = $1
returning
  id,
  username,
  display_name,
  avatar_url,
  player_color,
  registered_at,
  last_active_at,
  privacy_hide_activity;
