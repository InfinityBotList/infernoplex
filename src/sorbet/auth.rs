use sqlx::PgPool;

/// Represents a session that can be used to authorize/identify a user
#[allow(dead_code)]
pub struct Session {
    pub id: String,
    pub name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub r#type: String,
    pub target_type: String,
    pub target_id: String,
    pub perm_limits: Vec<String>,
    pub expiry: chrono::DateTime<chrono::Utc>,
}

impl Session {
    pub async fn clear_expired_sessions(pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query!("DELETE FROM api_sessions WHERE expiry < NOW()")
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn from_token(pool: &PgPool, token: &str) -> Result<Option<Self>, sqlx::Error> {
        // Before doing anything else, delete expired/old auths first
        Self::clear_expired_sessions(pool).await?;

        let session = sqlx::query_as!(
            Self,
            "SELECT id, name, created_at, type, target_type, target_id, perm_limits, expiry FROM api_sessions WHERE token = $1",
            token
        )
        .fetch_optional(pool)
        .await?;

        Ok(session)
    }
}
