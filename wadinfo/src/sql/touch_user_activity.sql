update user_profile
set last_active_at = $2
where id = $1;
