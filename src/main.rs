use std::path::{Path, PathBuf};

use anyhow::Result;

fn main() -> Result<()> {
    let repo = git2::Repository::open(".")?;

    let gerrit_private_key =
        PathBuf::from("/Users/sudeep.tarlekar/.ssh/id_ed25519_test_automation");
    let github_private_key = PathBuf::from("/Users/sudeep.tarlekar/.ssh/id_ed25519_github");

    let mut upstream_remote = repo.find_remote("upstream")?;
    let mut gerrit_remote = repo.find_remote("gerrit")?;

    if !(upstream_remote.connect(git2::Direction::Fetch).is_ok()
        && upstream_remote.connect(git2::Direction::Push).is_ok())
    {
        anyhow::bail!("Could not push or fetch from upstream remote");
    }

    if !(gerrit_remote.connect(git2::Direction::Fetch).is_ok()
        && gerrit_remote.connect(git2::Direction::Push).is_ok())
    {
        anyhow::bail!("Could not push or fetch from upstream remote");
    }

    let mut fetch_options = git2::FetchOptions::new();
    let cbs = setup_remote_callbacks(github_private_key.as_path())?;
    fetch_options.remote_callbacks(cbs);

    upstream_remote.fetch(&[] as &[&str], Some(&mut fetch_options), None)?;

    let mut push_options = git2::PushOptions::new();
    let cbs = setup_remote_callbacks(gerrit_private_key.as_path())?;
    push_options.remote_callbacks(cbs);

    let branches = repo
        .branches(Some(git2::BranchType::Remote))?
        .filter_map(Result::ok)
        .filter_map(|(branch, _branch_type)| branch.name().ok().flatten().map(String::from))
        .filter(|branch_name| branch_name.starts_with("upstream/"))
        .filter_map(|branch_name| branch_name.split("/").last().map(String::from))
        .collect::<Vec<String>>();

    println!("{branches:?}");

    for branch in branches {
        println!("Pushing branch {branch}...");
        let refspec = format!("refs/remotes/upstream/{branch}:refs/heads/{branch}");
        println!("branch mapping: {refspec}");

        match gerrit_remote.push(&[&refspec], Some(&mut push_options)) {
            Ok(_) => {
                println!("Force push completed successfully");
            }
            Err(e) => {
                eprintln!(
                    "Push error: {} (class: {:?}, code: {:?})",
                    e,
                    e.class(),
                    e.code()
                );

                // Specific diagnostics based on error code
                match e.code() {
                    git2::ErrorCode::BareRepo => {
                        eprintln!("Issue with bare repository configuration")
                    }
                    git2::ErrorCode::NotFound => eprintln!("Remote or reference not found"),
                    git2::ErrorCode::Auth => eprintln!("Authentication failed"),
                    _ => eprintln!("Other error type: {:?}", e.code()),
                }
            }
        }
    }
    Ok(())
}

fn setup_remote_callbacks<'ssh_key, 'callbacks>(
    private_key: &'ssh_key Path,
) -> Result<git2::RemoteCallbacks<'callbacks>>
where
    'ssh_key: 'callbacks,
{
    let mut cbs = git2::RemoteCallbacks::new();
    cbs.credentials(|_url, username, _allowed_types| {
        git2::Cred::ssh_key(username.unwrap(), None, private_key, None)
    });

    cbs.transfer_progress(|stats| {
        println!(
            "Transfer progress: {}/{} objects",
            stats.received_objects(),
            stats.total_objects()
        );
        true
    });

    Ok(cbs)
}
