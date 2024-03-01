use crate::instance::RuntimeInstance;
use crate::instance::{CreateRuntimeInstance, RuntimeInstanceRunCode};
use crate::AxumSharedState;
use crate::SharedState;
use axum::{
    extract::Path, extract::State, http::StatusCode, routing::get, routing::post, Json, Router,
};
use runtimelib::jupyter::client::JupyterRuntime;
use uuid::Uuid;

pub fn instance_routes() -> Router<SharedState> {
    Router::new()
        .route("/v0/runtime_instances", post(post_runtime_instance))
        .route("/v0/runtime_instances/:id", get(get_runtime_instance))
        .route("/v0/runtime_instances", get(get_runtime_instances))
        .route(
            "/v0/runtime_instances/:id/run_code",
            post(post_runtime_instance_run_code),
        )
}

async fn get_runtime_instances(
    State(state): AxumSharedState,
) -> Result<Json<Vec<JupyterRuntime>>, StatusCode> {
    let instances = sqlx::query_as!(
        RuntimeInstance,
        r#"SELECT id "id: uuid::Uuid", name FROM runtime_instances;"#
    )
    .fetch_all(&state.dbpool)
    .await
    .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;

    let runtimes = runtimelib::list_instances().await;

    Ok(Json(runtimes))
}

async fn get_runtime_instance(Path(id): Path<String>) -> Result<Json<JupyterRuntime>, StatusCode> {
    let runtimes = runtimelib::list_instances().await;
    let instance = runtimes
        .iter()
        .find(|runtime| runtime.connection_file.contains(&id))
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(instance.clone()))
}

async fn post_runtime_instance(
    State(state): AxumSharedState,
    Json(payload): Json<CreateRuntimeInstance>,
) -> Result<(StatusCode, Json<RuntimeInstance>), StatusCode> {
    let instance = RuntimeInstance {
        id: Uuid::new_v4(),
        name: payload.process,
    };

    sqlx::query_as!(
        RuntimeInstance,
        r#"INSERT INTO runtime_instances VALUES($1, $2)"#,
        instance.id,
        instance.name,
    )
    .execute(&state.dbpool)
    .await
    .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;

    Ok((StatusCode::CREATED, Json(instance)))
}

async fn post_runtime_instance_run_code(
    Path(id): Path<String>,
    State(state): AxumSharedState,
    Json(payload): Json<RuntimeInstanceRunCode>,
) -> Result<Json<JupyterRuntime>, StatusCode> {
    let runtimes = runtimelib::list_instances().await;
    let instance = runtimes
        .iter()
        .find(|runtime| runtime.connection_file.contains(&id))
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut client = instance
        .clone()
        .attach()
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;

    client
        .run_code(&payload.code)
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;

    Ok(Json(instance.clone()))
}