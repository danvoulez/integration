use std::future::Future;

tokio::task_local! {
    static REQUEST_ID: String;
}

pub async fn with_request_id_scope<F, T>(request_id: String, future: F) -> T
where
    F: Future<Output = T>,
{
    REQUEST_ID.scope(request_id, future).await
}

pub fn current_request_id() -> Option<String> {
    REQUEST_ID.try_with(|request_id| request_id.clone()).ok()
}
