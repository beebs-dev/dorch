pub fn user_record_key(username: &str) -> String {
    format!("auth:user:{}", username)
}
