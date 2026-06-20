use anyhow::Result;

use super::context::SourceContext;
use super::types::{SourceId, SourceRequest, SourceResponse};

pub trait SourceAdapter: Send + Sync {
    fn id(&self) -> SourceId;

    fn fetch(&self, ctx: &SourceContext, req: SourceRequest) -> Result<SourceResponse>;
}
