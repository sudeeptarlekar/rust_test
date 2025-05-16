use std::cell::RefCell;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

fn main() -> Result<()> {
    let repo = git2::Repository::open(".")?;
    let gerrit_private_key =
        PathBuf::from("/Users/sudeep.tarlekar/.ssh/id_ed25519_test_automation");
    let github_private_key = PathBuf::from("/Users/sudeep.tarlekar/.ssh/id_ed25519_github");

    let mut gerrit_remote = repo.find_remote("gerrit")?;
    let mut upstream_remote = repo.find_remote("upstream")?;

    fetch_remote(&mut gerrit_remote, gerrit_private_key.as_path())?;
    fetch_remote(&mut upstream_remote, github_private_key.as_path())?;

    let upstream_branches = repo
        .branches(Some(git2::BranchType::Remote))?
        .filter_map(Result::ok)
        .map(|(branch, _branch_type)| branch)
        .filter(|branch| {
            branch
                .name()
                .is_ok_and(|branch_name| branch_name.unwrap_or("").starts_with("upstream/"))
        })
        .collect::<Vec<git2::Branch>>();

    for branch in upstream_branches {
        let branch_name = branch
            .name()?
            .context("Missing branch name")?
            .split("/")
            .collect::<Vec<&str>>()
            .get(1..)
            .map(|x| x.join("/"))
            .context("Unable to get branch name")?;

        println!("Pushing branch from upstream : {branch_name}");

        let refspec = format!("refs/remotes/upstream/{branch_name}:refs/heads/{branch_name}");

        push_remote(&mut gerrit_remote, gerrit_private_key.as_path(), &refspec)?;
    }
    Ok(())
}

fn generate_push_options<'ssh_key, 'push_opts>(
    private_key: &'ssh_key Path,
) -> Result<git2::PushOptions<'push_opts>>
where
    'ssh_key: 'push_opts,
{
    let mut push_options = git2::PushOptions::new();
    let mut cbs = git2::RemoteCallbacks::new();

    cbs.credentials(|_url, username_from_url, _allowed_types| {
        git2::Cred::ssh_key(username_from_url.unwrap_or("git"), None, private_key, None)
    });

    push_options.remote_callbacks(cbs);

    Ok(push_options)
}

fn generate_fetch_options<'ssh_key, 'fetch_opts>(
    private_key: &'ssh_key Path,
) -> Result<git2::FetchOptions<'fetch_opts>>
where
    'ssh_key: 'fetch_opts,
{
    let mut fetch_options = git2::FetchOptions::new();
    let mut cbs = git2::RemoteCallbacks::new();

    cbs.credentials(|_url, username_from_url, _allowed_types| {
        git2::Cred::ssh_key(username_from_url.unwrap_or("git"), None, private_key, None)
    });

    fetch_options.remote_callbacks(cbs);

    Ok(fetch_options)
}

fn fetch_remote(remote: &mut git2::Remote, ssh_key: &Path) -> Result<()> {
    let mut fetch_options = generate_fetch_options(ssh_key)?;
    remote
        .fetch(&[] as &[&str], Some(&mut fetch_options), None)
        .with_context(|| format!("Could not fetch remote {:?}", remote.name()))
}

fn push_remote(remote: &mut git2::Remote, ssh_key: &Path, refspec: &str) -> Result<()> {
    let mut push_options = generate_push_options(ssh_key)?;

    remote
        .push(&[refspec], Some(&mut push_options))
        .context("Unable to push to remote")
}
