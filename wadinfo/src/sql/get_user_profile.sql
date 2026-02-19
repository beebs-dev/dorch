select
  id,
  username,
  avatar_url,
  registered_at,
  last_active_at,
  privacy_hide_activity
from user_profile
where id = $1;
