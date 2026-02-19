update user_profile
set
  display_name = $2,
  avatar_url = $3,
  last_active_at = $4,
  privacy_hide_activity = $5
where id = $1
returning
  id,
  username,
  display_name,
  avatar_url,
  registered_at,
  last_active_at,
  privacy_hide_activity;
