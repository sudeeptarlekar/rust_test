use anyhow::{bail, Context, Result};
use git2::{
    Commit, Config, Cred, Index, IndexAddOption, Oid, PushOptions, RemoteCallbacks, Repository,
    Signature, Tree,
};
use std::path::Path;

fn add_and_commit_changes(repo: &Repository) -> Result<Oid> {
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let oid = index
        .write_tree()
        .context("Error while writing the tree to the index")?;

    let tree = repo.find_tree(oid)?;
    let config = repo
        .config()
        .context("Could not get the config from the repo")?;
    // user.name and email cannot be empty; Git won't allow to set an empty username and email in
    // the config so no need to check the emptyness for these config values.
    let username = config
        .get_string("user.name")
        .context("Username is not set in the Git config; Please set username")?;
    let email = config
        .get_string("user.email")
        .context("Email is not set in the Git config; Please set email")?;
    let signature = Signature::now(&username, &email)
        .with_context(|| format!("Could not generate the signature from {username} and {email}"))?;
    let oid = if repo.head().is_ok() {
        let head_commit = repo.head()?.peel_to_commit()?;
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Added new function",
            &tree,
            &[&head_commit],
        )
        .context("Could not commit changes")?
    } else {
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial Commit",
            &tree,
            &[],
        )
        .context("Could not commit the changes to the repository")?
    };
    Ok(oid)
}

fn main() -> Result<()> {
    let repo = Repository::open(".")?;
    let oid = add_and_commit_changes(&repo)?;
    println!("{oid}");
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        Cred::ssh_key(
            username_from_url.unwrap(),
            None,
            Path::new("/Users/sudeep.tarlekar/.ssh/id_rsa"),
            None,
        )
    });

    let mut push_opts = PushOptions::new();
    push_opts.remote_callbacks(callbacks);
    let mut remote = repo.find_remote("origin")?;
    let refspecs = "refs/heads/master:refs/heads/master";

    remote
        .push(&[refspecs], Some(&mut push_opts))
        .context("Error while pushing changes to remote using ssh config")?;
    Ok(())
}
