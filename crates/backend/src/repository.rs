//! Generic repository traits for database operations.
//!
//! This module provides trait abstractions for common CRUD patterns,
//! reducing boilerplate while allowing entity-specific extensions.

use anyhow::Result;
use diesel_async::AsyncPgConnection;
use uuid::Uuid;

/// Core repository operations that most entities support.
///
/// This trait defines the common interface for database entities.
/// Entity-specific modules can implement this trait and add
/// additional methods as needed.
///
/// # Type Parameters
/// - `Entity`: The domain type returned from queries
/// - `CreateInput`: Input type for creating new entities
/// - `UpdateInput`: Input type for updating existing entities
#[allow(async_fn_in_trait)]
pub trait Repository {
    /// The domain entity type returned from database queries
    type Entity;

    /// Input type for creating new entities
    type CreateInput;

    /// Input type for updating existing entities
    type UpdateInput;

    /// List all entities, typically ordered by creation time descending.
    async fn list_all(conn: &mut AsyncPgConnection) -> Result<Vec<Self::Entity>>;

    /// Get a single entity by its UUID.
    async fn get_by_id(conn: &mut AsyncPgConnection, id: Uuid) -> Result<Self::Entity>;

    /// Create a new entity from the provided input.
    async fn create(conn: &mut AsyncPgConnection, input: Self::CreateInput)
        -> Result<Self::Entity>;

    /// Update an existing entity.
    async fn update(
        conn: &mut AsyncPgConnection,
        id: Uuid,
        input: Self::UpdateInput,
    ) -> Result<Self::Entity>;

    /// Delete an entity by ID.
    async fn delete(conn: &mut AsyncPgConnection, id: Uuid) -> Result<()>;
}

/// Marker trait for entities that support soft deletion.
///
/// Entities implementing this trait have an `is_active` field
/// and can be deactivated rather than deleted.
#[allow(async_fn_in_trait)]
pub trait SoftDeletable: Repository {
    /// List only active entities.
    async fn list_active(conn: &mut AsyncPgConnection) -> Result<Vec<Self::Entity>>;

    /// Soft-delete by setting is_active = false.
    async fn deactivate(conn: &mut AsyncPgConnection, id: Uuid) -> Result<Self::Entity>;
}

/// Marker trait for entities that track processing state.
///
/// Useful for entities like emails or events that need to be
/// processed by background workers.
#[allow(async_fn_in_trait)]
pub trait Processable: Repository {
    /// List entities that haven't been processed yet.
    async fn list_unprocessed(
        conn: &mut AsyncPgConnection,
        limit: i64,
    ) -> Result<Vec<Self::Entity>>;

    /// Mark an entity as processed.
    async fn mark_processed(conn: &mut AsyncPgConnection, id: Uuid) -> Result<()>;

    /// Count unprocessed entities.
    async fn count_unprocessed(conn: &mut AsyncPgConnection) -> Result<i64>;
}

/// Extension trait for entities that can be filtered by account.
#[allow(async_fn_in_trait)]
pub trait AccountScoped: Repository {
    /// List entities belonging to a specific account.
    async fn list_by_account(
        conn: &mut AsyncPgConnection,
        account_id: Uuid,
        limit: Option<i64>,
    ) -> Result<Vec<Self::Entity>>;
}

// ============================================================================
// Example Implementation: Categories
// ============================================================================
//
// This demonstrates how to implement the Repository trait for an entity.
// Other modules can follow this pattern.

/// Input for creating a new category
pub struct CreateCategoryInput<'a> {
    pub name: &'a str,
    pub color: Option<&'a str>,
}

/// Input for updating a category
pub struct UpdateCategoryInput<'a> {
    pub name: Option<&'a str>,
    pub color: Option<&'a str>,
}

/// Categories repository implementation
pub struct Categories;

impl Repository for Categories {
    type Entity = shared_types::Category;
    type CreateInput = CreateCategoryInput<'static>;
    type UpdateInput = UpdateCategoryInput<'static>;

    async fn list_all(conn: &mut AsyncPgConnection) -> Result<Vec<Self::Entity>> {
        crate::db::categories::list_all(conn).await
    }

    async fn get_by_id(conn: &mut AsyncPgConnection, id: Uuid) -> Result<Self::Entity> {
        crate::db::categories::get_by_id(conn, id).await
    }

    async fn create(
        conn: &mut AsyncPgConnection,
        input: Self::CreateInput,
    ) -> Result<Self::Entity> {
        crate::db::categories::create(conn, input.name, input.color).await
    }

    async fn update(
        conn: &mut AsyncPgConnection,
        id: Uuid,
        input: Self::UpdateInput,
    ) -> Result<Self::Entity> {
        crate::db::categories::update(conn, id, input.name, input.color).await
    }

    async fn delete(conn: &mut AsyncPgConnection, id: Uuid) -> Result<()> {
        crate::db::categories::delete(conn, id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Compile-time test that traits are object-safe enough to be used
    fn _assert_traits_exist() {
        fn _check_repository<T: Repository>() {}
        fn _check_soft_deletable<T: SoftDeletable>() {}
        fn _check_processable<T: Processable>() {}
        fn _check_account_scoped<T: AccountScoped>() {}
    }

    // Verify Categories implements Repository
    fn _check_categories_impl() {
        fn _check<T: Repository>() {}
        _check::<Categories>();
    }
}
