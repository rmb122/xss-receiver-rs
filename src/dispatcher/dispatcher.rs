use anyhow::Result;
use axum::{body::Body, http::Request, response::Response};
use once_cell::sync::Lazy;
use regex::RegexSet;
use tokio::sync::RwLock;

use super::route::Route;

pub static THE_DISPATCHER: Lazy<RwLock<Dispatcher>> = Lazy::new(|| {
    RwLock::new(Dispatcher {
        routes: vec![],
        route_regex_set: RegexSet::empty(),
    })
});

pub struct Dispatcher {
    routes: Vec<Route>,
    route_regex_set: RegexSet,
}

impl Dispatcher {
    pub fn compile_routes(&mut self, routes: Vec<Route>) -> Result<()> {
        self.routes = routes;
        self.route_regex_set = RegexSet::new(self.routes.iter().map(|x| x.pattern.clone()))?;

        Ok(())
    }

    pub async fn dispatch(&self, request: Request<Body>) -> Response<Body> {
        let path = request.uri().path();

        if let Some(idx) = self.route_regex_set.matches(path).iter().next() {
            if let Result::Ok(response) = self.routes[idx].handler.handle(request).await {
                return response;
            }
        }

        Response::builder().status(404).body(Body::empty()).unwrap()
    }
}
