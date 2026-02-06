use std::sync::Arc;

use anyhow::Result;
use axum::{body::Body, http::Request};
use regex::RegexSet;

use super::route::Route;

pub struct Dispatcher {
    // 这里使用 Arc, 因为在处理 route 的时候, 需要将 route clone 出来
    // 否则需要一直锁住 dispacher, 导致如果有一个 handler 速度比较慢的话, 无法更新该 dispacher (死锁)
    routes: Vec<Arc<Route>>,
    route_regex_set: RegexSet,
}

impl Dispatcher {
    pub fn new(routes: Vec<Route>) -> Result<Self> {
        let route_regex_set = RegexSet::new(routes.iter().map(|x| x.pattern.clone()))?;

        Ok(Dispatcher {
            routes: routes.into_iter().map(|x| Arc::new(x)).collect(),
            route_regex_set: route_regex_set,
        })
    }

    pub fn dispatch(&self, request: &Request<Body>) -> Option<Arc<Route>> {
        let path = request.uri().path();

        if let Some(idx) = self.route_regex_set.matches(path).iter().next() {
            return Some(self.routes[idx].clone());
        }

        None
    }
}
