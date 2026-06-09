use energy_api::{handler, AppState};
use lambda_http::{run, service_fn, Error};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let state = AppState::from_env().await?;
    run(service_fn(|event| handler(event, state.clone()))).await
}
