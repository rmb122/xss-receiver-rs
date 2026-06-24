use std::sync::Arc;

use anyhow::Result;
use regex::RegexSet;

use super::{dns_route::DnsRoute, http_route::HttpRoute};

pub trait DispatchRoute: Sync + Send {
    fn pattern(&self) -> &str;
    fn priority(&self) -> i32;
}

pub type HttpDispatcher = RouteDispatcher<HttpRoute>;
pub type DnsDispatcher = RouteDispatcher<DnsRoute>;

pub struct RouteDispatcher<R> {
    // 这里使用 Arc, 因为在处理 route 的时候, 需要将 route clone 出来
    // 否则需要一直锁住 dispacher, 导致如果有一个 handler 速度比较慢的话, 无法更新该 dispacher (死锁)
    routes: Vec<Arc<R>>,
    route_regex_set: RegexSet,
}

impl<R: DispatchRoute> RouteDispatcher<R> {
    pub fn new(routes: Vec<R>) -> Result<Self> {
        let route_regex_set = RegexSet::new(routes.iter().map(|x| x.pattern()))?;

        Ok(RouteDispatcher {
            routes: routes.into_iter().map(Arc::new).collect(),
            route_regex_set,
        })
    }

    pub fn dispatch_key(&self, key: &str) -> Option<Arc<R>> {
        if let Some(max_idx) =
            self.route_regex_set
                .matches(key)
                .iter()
                .fold(None, |max: Option<usize>, x: usize| {
                    match max {
                        // 返回 priority 最大的 route
                        None => Some(x),
                        Some(max) => {
                            if self.routes[max].priority() < self.routes[x].priority() {
                                Some(x)
                            } else {
                                Some(max)
                            }
                        }
                    }
                })
        {
            return Some(self.routes[max_idx].clone());
        }

        None
    }
}
