use anyhow::Result;
use rmcp::{transport::io::stdio, ServiceExt};
use tokio::task;
use tracing_subscriber::{self, filter::EnvFilter};

use mcp_server::start;

#[tokio::main]
async fn main() -> Result<()> {
    let (ai_window, mut server_comm, _client_comm) = start()?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::DEBUG.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Runtime: Starting MCP Server");

    let service = ai_window
        .serve(stdio())
        .await
        .inspect_err(|err| tracing::error!("Error while starting the AI Server: {err:?}"))?;

    let server_task = task::spawn(service.waiting());
    let receiver_task = task::spawn(async move {
        while let Some(task) = server_comm.task_rx.recv().await {
            tracing::debug!("Runtime: Received Task with parameters: {task:?}");
        }
    });

    let (server_task_result, receiver_task_result) = tokio::join!(server_task, receiver_task);
    match server_task_result {
        Ok(Ok(reason)) => tracing::debug!("Shutting down server for reason: {reason:?}"),
        Ok(Err(err)) => tracing::error!("Runtime: Error while starting the server; {err:?}"),
        _ => tracing::error!("Runtime: Error while joining threads in runtime"),
    }

    match receiver_task_result {
        Ok(_) => tracing::debug!("Runtime: Receiver task ended successfully"),
        Err(err) => tracing::error!("Runtime: Error in receiver task; {err:?}"),
    }

    tracing::info!("Runtime: Server started and now waiting for the message to receive");

    Ok(())
}
