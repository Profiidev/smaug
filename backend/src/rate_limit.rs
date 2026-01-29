use std::{
  thread::{sleep, spawn},
  time::Duration,
};

use governor::middleware::StateInformationMiddleware;
use tower_governor::{
  governor::{GovernorConfig, GovernorConfigBuilder},
  key_extractor::SmartIpKeyExtractor,
};

pub type Governor = GovernorConfig<SmartIpKeyExtractor, StateInformationMiddleware>;

#[derive(Default)]
pub struct RateLimiter {
  cleaner: Vec<Box<dyn Fn() + Send + Sync>>,
}

impl RateLimiter {
  pub fn create_limiter(&mut self) -> Governor {
    let conf = GovernorConfigBuilder::default()
      .key_extractor(SmartIpKeyExtractor)
      .per_second(10)
      .burst_size(20)
      .use_headers()
      .finish()
      .unwrap();

    let limiter = conf.limiter().clone();
    self.cleaner.push(Box::new(move || {
      limiter.retain_recent();
    }));

    conf
  }

  pub fn init(self) {
    spawn(move || {
      loop {
        sleep(Duration::from_secs(600));
        tracing::debug!("Cleaning rate limiter state");
        for clean in &self.cleaner {
          clean();
        }
      }
    });
  }
}
