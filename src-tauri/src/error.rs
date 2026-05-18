use smart_noter_core::AppError;
use smart_noter_db::DbError;

pub fn from_db(e: DbError) -> AppError {
    AppError::Database(e.to_string())
}
