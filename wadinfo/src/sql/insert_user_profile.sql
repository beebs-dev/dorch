insert into user_profile (
  id,
  username,
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
  null,
  $5
)
returning
  id,
  username,
  avatar_url,
  registered_at,
  last_active_at,
  privacy_hide_activity;
