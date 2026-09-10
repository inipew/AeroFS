use crate::errors::AppError;
use axum::{
    extract::{
        rejection::{JsonRejection, PathRejection, QueryRejection},
        FromRequest, FromRequestParts, Request,
    },
    http::request::Parts,
};
use serde::de::DeserializeOwned;
use std::ops::{Deref, DerefMut};

/// Extractor for JSON request bodies that converts `JsonRejection` into `AppError::BadRequest`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Json<T>(pub T);

impl<T> Deref for Json<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Json<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: serde::Serialize> axum::response::IntoResponse for Json<T> {
    fn into_response(self) -> axum::response::Response {
        axum::Json(self.0).into_response()
    }
}

impl<S, T> FromRequest<S> for Json<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(val)) => Ok(Json(val)),
            Err(rejection) => {
                if rejection.status() == axum::http::StatusCode::PAYLOAD_TOO_LARGE {
                    Err(AppError::PayloadTooLarge(rejection.body_text()))
                } else {
                    Err(AppError::BadRequest(rejection.body_text()))
                }
            }
        }
    }
}

/// Extractor for URL query parameters that converts `QueryRejection` into `AppError::BadRequest`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

impl<T> Deref for Query<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Query<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<S, T> FromRequestParts<S> for Query<T>
where
    axum::extract::Query<T>: FromRequestParts<S, Rejection = QueryRejection>,
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Query::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Query(val)) => Ok(Query(val)),
            Err(rejection) => Err(AppError::BadRequest(rejection.body_text())),
        }
    }
}

/// Extractor for URL path parameters that converts `PathRejection` into `AppError::BadRequest`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Path<T>(pub T);

impl<T> Deref for Path<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Path<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<S, T> FromRequestParts<S> for Path<T>
where
    axum::extract::Path<T>: FromRequestParts<S, Rejection = PathRejection>,
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Path(val)) => Ok(Path(val)),
            Err(rejection) => Err(AppError::BadRequest(rejection.body_text())),
        }
    }
}
