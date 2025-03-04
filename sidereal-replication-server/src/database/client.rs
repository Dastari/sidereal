use bevy::prelude::*;
use dotenv::dotenv;
use reqwest::{self, header};
use serde::{Deserialize, Serialize};
use std::env;
use thiserror::Error;

/// Error types for database operations
#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Environment variable not set: {0}")]
    EnvVarError(#[from] std::env::VarError),

    #[error("Invalid header value: {0}")]
    HeaderValueError(#[from] reqwest::header::InvalidHeaderValue),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Not found")]
    NotFound,

    #[error("HTTP error: {0}")]
    HttpError(u16),
}

/// Result type for database operations
pub type DatabaseResult<T> = Result<T, DatabaseError>;

/// Database record for an entity
#[derive(Debug, Serialize, Deserialize)]
pub struct EntityRecord {
    pub id: String,
    pub name: Option<String>,
    pub owner_id: Option<String>,
    pub position_x: f32,
    pub position_y: f32,
    #[serde(rename = "type")]
    pub type_: String,
    pub components: serde_json::Value,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,

}

/// Client for interacting with the Supabase database
#[derive(Resource)]
pub struct DatabaseClient {
    client: reqwest::Client,
    pub base_url: String,
}

impl DatabaseClient {
    /// Creates a new database client
    pub fn new() -> DatabaseResult<Self> {
        dotenv().ok(); // Load .env file

        let base_url = env::var("SUPABASE_URL")?;
        let anon_key = env::var("ANON_KEY")?;

        let mut headers = header::HeaderMap::new();
        headers.insert("apikey", header::HeaderValue::from_str(&anon_key)?);
        headers.insert(
            "Authorization",
            header::HeaderValue::from_str(&format!("Bearer {}", anon_key))?,
        );
        headers.insert(
            "Content-Type",
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            "X-Consumer-Username",
            header::HeaderValue::from_static("service_role"),
        );
        headers.insert(
            "X-Consumer-Groups",
            header::HeaderValue::from_static("admin"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(DatabaseError::NetworkError)?;

        Ok(DatabaseClient { client, base_url })
    }

    /// Fetches all entities from the database
    pub async fn fetch_all_entities(&self) -> DatabaseResult<Vec<EntityRecord>> {
        let url = format!("{}/rest/v1/entities", self.base_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(DatabaseError::HttpError(response.status().as_u16()));
        }

        let entities = response.json().await?;
        Ok(entities)
    }

    /// Fetches entities by type from the database
    #[allow(dead_code)]
    pub async fn fetch_entities_by_type(
        &self,
        entity_type: &str,
    ) -> DatabaseResult<Vec<EntityRecord>> {
        let url = format!("{}/rest/v1/entities?type=eq.{}", self.base_url, entity_type);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(DatabaseError::HttpError(response.status().as_u16()));
        }

        let entities = response.json().await?;
        Ok(entities)
    }

    /// Fetches a single entity by ID from the database
    #[allow(dead_code)]
    pub async fn fetch_entity_by_id(&self, entity_id: &str) -> DatabaseResult<EntityRecord> {
        let url = format!("{}/rest/v1/entities?id=eq.{}", self.base_url, entity_id);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(DatabaseError::HttpError(response.status().as_u16()));
        }

        let mut entities: Vec<EntityRecord> = response.json().await?;

        match entities.pop() {
            Some(entity) => Ok(entity),
            None => Err(DatabaseError::NotFound),
        }
    }

    /// Creates a new entity in the database
    #[allow(dead_code)]
    pub async fn create_entity(&self, entity: &EntityRecord) -> DatabaseResult<()> {
        let url = format!("{}/rest/v1/entities", self.base_url);
        let response = self.client.post(&url).json(entity).send().await?;

        if !response.status().is_success() {
            return Err(DatabaseError::HttpError(response.status().as_u16()));
        }

        Ok(())
    }

    /// Updates an entity in the database
    #[allow(dead_code)]
    pub async fn update_entity(
        &self,
        entity_id: &str,
        entity: &EntityRecord,
    ) -> DatabaseResult<()> {
        let url = format!("{}/rest/v1/entities?id=eq.{}", self.base_url, entity_id);
        let response = self.client.patch(&url).json(entity).send().await?;

        if !response.status().is_success() {
            return Err(DatabaseError::HttpError(response.status().as_u16()));
        }

        Ok(())
    }

    /// Deletes an entity from the database
    #[allow(dead_code)]
    pub async fn delete_entity(&self, entity_id: &str) -> DatabaseResult<()> {
        let url = format!("{}/rest/v1/entities?id=eq.{}", self.base_url, entity_id);
        let response = self.client.delete(&url).send().await?;

        if !response.status().is_success() {
            return Err(DatabaseError::HttpError(response.status().as_u16()));
        }

        Ok(())
    }
}
