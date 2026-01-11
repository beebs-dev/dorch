use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct NatsArgs {
    #[arg(long, env = "NATS_URL", required = true)]
    pub nats_url: String,

    #[arg(long, env = "NATS_USER", default_value = "app")]
    pub nats_user: String,

    #[arg(long, env = "NATS_PASSWORD", default_value = "devpass")]
    pub nats_password: String,

    #[arg(long, env = "NATS_CONSUMER_REPLICAS", default_value_t = 1)]
    pub consumer_replicas: usize,

    #[arg(long, env = "NATS_MAX_DELIVER", default_value_t = 5)]
    pub max_deliver: i64,

    #[arg(long, env = "NATS_MAX_ACK_PENDING", default_value_t = 2)]
    pub max_ack_pending: i64,
}

#[derive(Parser, Debug, Clone)]
pub struct DatabaseArgs {
    #[arg(long, default_value = "postgres")]
    pub db: String,

    #[clap(flatten)]
    pub postgres: PostgresArgs,
}

#[derive(Parser, Debug, Clone)]
pub struct PostgresArgs {
    #[arg(long, env = "POSTGRES_HOST", default_value = "localhost")]
    pub postgres_host: String,

    #[arg(long, env = "POSTGRES_PORT", default_value_t = 5432)]
    pub postgres_port: u16,

    #[arg(long, env = "POSTGRES_DATABASE", default_value = "postgres")]
    pub postgres_database: String,

    #[arg(long, env = "POSTGRES_USERNAME", default_value = "postgres")]
    pub postgres_username: String,

    #[arg(long, env = "POSTGRES_PASSWORD")]
    pub postgres_password: Option<String>,

    #[arg(long, env = "POSTGRES_CA_CERT")]
    pub postgres_ca_cert: Option<String>,

    #[arg(long, env = "POSTGRES_SSL_MODE", default_value = "prefer")]
    pub postgres_ssl_mode: String,
}

#[derive(Parser, Debug, Clone)]
pub struct RedisArgs {
    #[arg(long, env = "REDIS_HOST", default_value = "127.0.0.1")]
    pub redis_host: String,

    #[arg(long, env = "REDIS_PORT", default_value_t = 6379)]
    pub redis_port: u16,

    #[arg(long, env = "REDIS_USERNAME")]
    pub redis_username: Option<String>,

    #[arg(long, env = "REDIS_PASSWORD")]
    pub redis_password: Option<String>,

    #[arg(long, env = "REDIS_PROTO", default_value = "redis")]
    pub redis_proto: String,
}

impl RedisArgs {
    pub fn url_redacted(&self) -> String {
        format!(
            "{}://{}:{}@{}:{}",
            if self.redis_proto.is_empty() {
                "redis"
            } else {
                &self.redis_proto
            },
            self.redis_username.as_deref().unwrap_or(""),
            self.redis_password.as_deref().map(|_| "****").unwrap_or(""),
            self.redis_host,
            self.redis_port
        )
    }

    pub fn url(&self) -> String {
        let proto = if self.redis_proto.is_empty() {
            "redis"
        } else {
            &self.redis_proto
        };
        let mut url = format!("{}://", proto);
        if let Some(ref username) = self.redis_username {
            url.push_str(username);
            if let Some(ref password) = self.redis_password {
                url.push(':');
                url.push_str(password);
            }
            url.push('@');
        } else if let Some(ref password) = self.redis_password {
            url.push(':');
            url.push_str(password);
            url.push('@');
        }
        url.push_str(&format!("{}:{}/", self.redis_host, self.redis_port));
        url
    }
}

#[derive(Parser, Debug, Clone)]
pub struct KeycloakArgs {
    #[arg(long, env = "KC_ENDPOINT", required = true)]
    pub endpoint: String,

    #[arg(long, env = "KC_REALM", required = true)]
    pub realm: String,

    #[arg(long, env = "KC_ADMIN_REALM", required = true)]
    pub admin_realm: String,

    #[arg(long, env = "KC_ADMIN_USERNAME", required = true)]
    pub admin_username: String,

    #[arg(long, env = "KC_ADMIN_PASSWORD", required = true)]
    pub admin_password: String,

    #[arg(long, env = "KC_CLIENT_ID", required = true)]
    pub client_id: String,

    #[arg(long, env = "KC_CLIENT_SECRET", required = true)]
    pub client_secret: String,
}
