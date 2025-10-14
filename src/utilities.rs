use std::path::Path;

use anyhow::{Context, Result};
use git2::Repository;

pub fn add_fork_remote(repo: &Repository) -> Result<git2::Remote<'_>> {
    if let Ok(remote) = repo.find_remote("fork") {
        Ok(remote)
    } else {
        repo.remote("fork", "git@github.com:sudeeptarlekar/playground.git")
            .context("Can not add remote 'fork'")
    }
}

pub fn clone_or_update_repo(
    repo_path: &Path,
    ssh_key: &Path,
    repo_url: &str,
) -> Result<git2::Repository> {
    if repo_path.exists() {
        let repo = git2::Repository::open(repo_path)?;
        fetch_repo(&repo, ssh_key)?;
        Ok(repo)
    } else {
        let mut fetch_options = git2::FetchOptions::new();
        let mut cbs = git2::RemoteCallbacks::new();
        cbs.credentials(|_url, username, _at| {
            git2::Cred::ssh_key(username.unwrap(), None, ssh_key, None)
        });
        fetch_options.remote_callbacks(cbs);

        let mut repo_builder = git2::build::RepoBuilder::new();
        repo_builder.bare(true);
        repo_builder.fetch_options(fetch_options);
        repo_builder
            .clone(repo_url, repo_path)
            .context("Could not clone a repo")
    }
}

pub fn delete_merged_pr_branch(
    repo: &Repository,
    ssh_key: &Path,
    merged_branches: Vec<String>,
) -> Result<()> {
    println!("Deleting merged branches");
    fetch_repo(repo, ssh_key)?;
    let branches = list_remote_branches(repo)?;
    for mut branch in branches {
        let branch_name = get_branch_name(&branch)?;

        if merged_branches.contains(&branch_name) {
            branch.delete()?;
            let refspec = format!(":refs/heads/{branch_name}");
            push_remote_reference(repo, ssh_key, &refspec)?;
        }
    }
    println!("Finished merged branches check");
    Ok(())
}

fn push_remote_reference(repo: &Repository, ssh_key: &Path, refspec: &str) -> Result<()> {
    let mut remote = repo.find_remote("fork")?;

    let mut push_options = git2::PushOptions::new();
    let mut cbs = git2::RemoteCallbacks::new();

    cbs.credentials(|_url, username, _at| {
        git2::Cred::ssh_key(username.unwrap(), None, ssh_key, None)
    });
    push_options.remote_callbacks(cbs);

    remote
        .push(&[refspec], Some(&mut push_options))
        .context("Can not push delete branch to remote fork")
}

fn get_branch_name(branch: &git2::Branch) -> Result<String> {
    branch
        .name()?
        .map(|name| name.split("/").last().unwrap_or(name))
        .map(String::from)
        .ok_or(anyhow::Error::msg("Not a valid branch name"))
}

fn list_remote_branches(repo: &Repository) -> Result<Vec<git2::Branch<'_>>> {
    Ok(repo
        .branches(Some(git2::BranchType::Remote))?
        .filter_map(|branch| branch.ok())
        .map(|(branch, _branch_type)| branch)
        .collect())
}

fn fetch_repo(repo: &Repository, ssh_key: &Path) -> Result<()> {
    let mut fetch_options = git2::FetchOptions::new();
    let mut remote_callbacks = git2::RemoteCallbacks::new();
    remote_callbacks.credentials(|_url, username, _at| {
        git2::Cred::ssh_key(username.unwrap(), None, ssh_key, None)
    });
    fetch_options.remote_callbacks(remote_callbacks);

    let _ = repo
        .remotes()?
        .iter()
        .filter_map(|remote_name| remote_name.and_then(|name| repo.find_remote(name).ok()))
        .map(|mut remote| {
            remote
                .fetch(&[""] as &[&str], Some(&mut fetch_options), None)
                .context("Could not fetch remote")
        })
        .collect::<Result<Vec<()>>>()?;
    Ok(())
}
