use std::path::PathBuf;

use anyhow::{Context, Result};
use git2::{BranchType, Cred, FetchOptions, PushOptions, RemoteCallbacks, Repository};

fn main() -> Result<()> {
    let ssh_key_path: PathBuf = PathBuf::from("/Users/sudeep.tarlekar/.ssh/id_rsa_github");

    let repo = Repository::open(".").context("Current directory is not a git Repo")?;
    let branch = repo.find_branch("test_branch", BranchType::Local)?;
    let branch_ref = branch.get();

    let mut remote = repo
        .find_remote("origin")
        .context("Could not find remote `origin` in Git repo")?;

    let mut remote_callbacks = RemoteCallbacks::new();
    remote_callbacks.credentials(|_url, username_from_url, _allowed_types| {
        Cred::ssh_key(
            username_from_url.unwrap(),
            None,
            ssh_key_path.as_ref(),
            None,
        )
    });

    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(remote_callbacks);

    remote
        .push(
            &["refs/heads/test_branch:refs/remotes/test_branch"],
            Some(&mut push_options),
        )
        .context("Unable to push to `test_branch`")?;

    println!("I'm done!!!");
    Ok(())
}
