use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;

#[derive(Serialize, utoipa::ToSchema)]
pub struct Response<T: Serialize> {
    code: i32,
    msg: Option<String>,
    payload: Option<T>,
}

impl<T: Serialize> Response<T> {
    pub fn new() -> Response<T> {
        Response {
            code: 0,
            msg: None,
            payload: None,
        }
    }

    pub fn ok() -> Response<T> {
        Response {
            code: 200,
            msg: None,
            payload: None,
        }
    }

    pub fn error<M: Into<String>>(msg: M) -> Response<T> {
        Response {
            code: 500,
            msg: Some(msg.into()),
            payload: None,
        }
    }

    pub fn code(mut self, code: i32) -> Response<T> {
        self.code = code;
        self
    }

    pub fn msg<M: Into<String>>(mut self, msg: M) -> Response<T> {
        self.msg = Some(msg.into());
        self
    }

    pub fn payload(mut self, payload: T) -> Response<T> {
        self.payload = Some(payload);
        self
    }
}

impl<T: Serialize> IntoResponse for Response<T> {
    fn into_response(self) -> axum::http::Response<axum::body::Body> {
        (StatusCode::OK, Json(self)).into_response()
    }
}
