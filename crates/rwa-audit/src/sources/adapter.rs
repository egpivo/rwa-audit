use anyhow::Result;

use super::cache::ResponseCache;
use super::transport::HttpTransport;
use super::types::{SourceId, SourceRequest, SourceResponse};

pub trait SourceAdapter {
    fn id(&self) -> SourceId;

    fn fetch(
        &self,
        transport: &HttpTransport,
        cache: &ResponseCache,
        req: SourceRequest,
    ) -> Result<SourceResponse>;
}
