select
  id,
  username,
  display_name,
  avatar_url,
  player_color,
  registered_at,
  last_active_at,
  privacy_hide_activity
from user_profile
where id = $1;
