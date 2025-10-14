pub mod pull_request;
pub mod utilities;

use std::path::Path;

use anyhow::{Context, Result};
use tokio::signal;
use tokio::time::{sleep, Duration};

use pull_request::fetch_pull_requests;
use utilities::{add_fork_remote, clone_or_update_repo, delete_merged_pr_branch};

#[tokio::main]
async fn main() -> Result<()> {
    let timeout = Duration::from_secs(10);
    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                println!("Ctrl+c received, shutting down app");
                break;
            },
            _ = sleep(timeout) => {
                println!("Timeout Reached, rechecking");
                main_task().await?;
            }
        }
    }
    Ok(())
}

async fn main_task() -> Result<()> {
    let ssh_key = Path::new("/Users/sudeep.tarlekar/.ssh/id_ed25519_github");
    let repo_path = Path::new("/Users/sudeep.tarlekar/tmp/playground");
    let repo_url = String::from("git@github.com:marcmo/playground.git");

    let repo = clone_or_update_repo(repo_path, ssh_key, &repo_url)?;
    let _fork_remote = add_fork_remote(&repo)?;
    clone_or_update_repo(repo_path, ssh_key, &repo_url)?;
    let merged_branches = fetch_pull_requests()
                .await
                .context("Error while fetching merged pull request from GitHub for repo 'playground' using GQL API")?
                .iter()
                .map(|pr| pr.branch_name.to_owned())
                .collect::<Vec<String>>();
    delete_merged_pr_branch(&repo, ssh_key, merged_branches)
        .context("Error while deleting merged PR branch on GitHub repo 'playground'")?;
    println!("Finished main task; sleeping");
    Ok(())
}
