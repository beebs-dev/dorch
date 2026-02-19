insert into user_profile (
  id,
  username,
  display_name,
  avatar_url,
  registered_at,
  last_active_at,
  privacy_hide_activity
)
values (
  $1,
  $2,
  $3,
  $4,
  $5,
  null,
  $6
)
returning
  id,
  username,
  display_name,
  avatar_url,
  registered_at,
  last_active_at,
  privacy_hide_activity;
