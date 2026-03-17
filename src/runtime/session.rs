use std::future::Future;

use anyhow::Result;

use crate::{
    app::AppEvent,
    config::paths::AppPaths,
    soundcloud::{auth, auth::AuthorizedSession, service::SoundcloudService},
};

pub(super) async fn execute<F, Fut>(
    paths: AppPaths,
    service: SoundcloudService,
    mut session: AuthorizedSession,
    run: F,
) -> Result<AppEvent>
where
    F: FnOnce(SoundcloudService, AuthorizedSession) -> Fut,
    Fut: Future<Output = Result<AppEvent>>,
{
    session.tokens =
        auth::ensure_fresh_tokens(&paths, &session.credentials, &session.tokens).await?;
    run(service, session).await
}
