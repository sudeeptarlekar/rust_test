use git2::{Cred, CredentialType, FetchOptions, PushOptions, RemoteCallbacks, Repository};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SSHKeyPair {
    pub name: String,
    pub private_key_path: PathBuf,
    pub public_key_path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct SSHKeyAuthenticator {
    ssh_keys: Vec<SSHKeyPair>,
    attempted_keys: HashMap<String, bool>,
}

impl SSHKeyAuthenticator {
    /// Creates a new SSH key authenticator by scanning the ~/.ssh directory
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let ssh_keys = Self::discover_ssh_keys()?;
        Ok(Self {
            ssh_keys,
            attempted_keys: HashMap::new(),
        })
    }

    /// Creates a new authenticator with a custom SSH directory
    pub fn with_ssh_dir<P: AsRef<Path>>(ssh_dir: P) -> Result<Self, Box<dyn std::error::Error>> {
        let ssh_keys = Self::discover_ssh_keys_in_dir(ssh_dir)?;
        Ok(Self {
            ssh_keys,
            attempted_keys: HashMap::new(),
        })
    }

    /// Discovers SSH keys in the default ~/.ssh directory
    pub fn discover_ssh_keys() -> Result<Vec<SSHKeyPair>, Box<dyn std::error::Error>> {
        let ssh_dir = dirs::home_dir()
            .ok_or("Could not determine home directory")?
            .join(".ssh");

        Self::discover_ssh_keys_in_dir(ssh_dir)
    }

    /// Discovers SSH keys in a specified directory
    pub fn discover_ssh_keys_in_dir<P: AsRef<Path>>(
        ssh_dir: P,
    ) -> Result<Vec<SSHKeyPair>, Box<dyn std::error::Error>> {
        let ssh_dir = ssh_dir.as_ref();

        if !ssh_dir.exists() {
            return Ok(Vec::new());
        }

        let mut keys = Vec::new();
        let entries = fs::read_dir(ssh_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    // Skip public keys, known_hosts, config, etc.
                    if file_name.ends_with(".pub")
                        || file_name == "known_hosts"
                        || file_name == "config"
                        || file_name == "authorized_keys"
                        || file_name.starts_with('.')
                    {
                        continue;
                    }

                    // Check if this looks like a private key
                    if Self::is_likely_private_key(&path)? {
                        let public_key_path = ssh_dir.join(format!("{}.pub", file_name));
                        let public_key_exists = public_key_path.exists();

                        keys.push(SSHKeyPair {
                            name: file_name.to_string(),
                            private_key_path: path,
                            public_key_path: if public_key_exists {
                                Some(public_key_path)
                            } else {
                                None
                            },
                        });
                    }
                }
            }
        }

        // Sort keys by common naming patterns (prioritize commonly used keys)
        keys.sort_by(|a, b| {
            let priority_a = Self::key_priority(&a.name);
            let priority_b = Self::key_priority(&b.name);
            priority_a.cmp(&priority_b)
        });

        Ok(keys)
    }

    /// Determines if a file is likely a private SSH key
    fn is_likely_private_key(path: &Path) -> Result<bool, std::io::Error> {
        let content = fs::read_to_string(path)?;
        let content_trimmed = content.trim();

        Ok(
            content_trimmed.starts_with("-----BEGIN OPENSSH PRIVATE KEY-----") ||
            content_trimmed.starts_with("-----BEGIN RSA PRIVATE KEY-----") ||
            content_trimmed.starts_with("-----BEGIN DSA PRIVATE KEY-----") ||
            content_trimmed.starts_with("-----BEGIN EC PRIVATE KEY-----") ||
            content_trimmed.starts_with("-----BEGIN PRIVATE KEY-----") ||
            // PuTTY format
            content_trimmed.starts_with("PuTTY-User-Key-File-"),
        )
    }

    /// Assigns priority to keys based on common naming patterns
    fn key_priority(name: &str) -> u8 {
        match name {
            "id_ed25519" => 1,
            "id_rsa" => 2,
            "id_ecdsa" => 3,
            "id_dsa" => 4,
            _ if name.starts_with("id_") => 5,
            _ => 10,
        }
    }

    /// Creates RemoteCallbacks that try all SSH keys
    pub fn create_callbacks(&mut self) -> RemoteCallbacks<'_> {
        let mut callbacks = RemoteCallbacks::new();

        callbacks.credentials(move |_url, username_from_url, allowed_types| {
            // Try SSH agent first
            if allowed_types.contains(CredentialType::SSH_KEY) {
                if let Some(username) = username_from_url {
                    if let Ok(cred) = Cred::ssh_key_from_agent(username) {
                        return Ok(cred);
                    }
                }
            }

            // Try each SSH key
            let username = username_from_url.unwrap_or("git");
            self.try_next_ssh_key(username)
        });

        callbacks
    }

    /// Tries the next available SSH key
    fn try_next_ssh_key(&mut self, username: &str) -> Result<Cred, git2::Error> {
        for key in &self.ssh_keys {
            // Skip if we've already tried this key
            if self.attempted_keys.contains_key(&key.name) {
                continue;
            }

            // Mark this key as attempted
            self.attempted_keys.insert(key.name.clone(), true);

            println!("Trying SSH key: {}", key.name);

            // Try without passphrase first
            match self.create_credential_for_key(key, username, None) {
                Ok(cred) => return Ok(cred),
                Err(_) => {
                    // If it fails, try with passphrase
                    if let Some(passphrase) = self.prompt_for_passphrase(&key.name) {
                        if let Ok(cred) =
                            self.create_credential_for_key(key, username, Some(&passphrase))
                        {
                            return Ok(cred);
                        }
                    }
                }
            }
        }

        Err(git2::Error::from_str("No valid SSH keys found"))
    }

    /// Creates a Git credential for a specific SSH key
    fn create_credential_for_key(
        &self,
        key: &SSHKeyPair,
        username: &str,
        passphrase: Option<&str>,
    ) -> Result<Cred, git2::Error> {
        Cred::ssh_key(
            username,
            key.public_key_path.as_deref(),
            &key.private_key_path,
            passphrase,
        )
    }

    /// Prompts for SSH key passphrase
    fn prompt_for_passphrase(&self, key_name: &str) -> Option<String> {
        print!(
            "Enter passphrase for SSH key '{}' (or press Enter to skip): ",
            key_name
        );
        io::stdout().flush().ok()?;

        // Use rpassword crate for hidden input if available
        match rpassword::read_password() {
            Ok(passphrase) if !passphrase.trim().is_empty() => Some(passphrase),
            _ => None,
        }
    }

    /// Resets the attempted keys cache
    pub fn reset(&mut self) {
        self.attempted_keys.clear();
    }

    /// Lists all discovered SSH keys
    pub fn list_keys(&self) -> &[SSHKeyPair] {
        &self.ssh_keys
    }
}

/// Convenience function to clone a repository trying all SSH keys
pub fn clone_with_ssh_keys<P: AsRef<Path>>(
    url: &str,
    path: P,
) -> Result<Repository, Box<dyn std::error::Error>> {
    let mut authenticator = SSHKeyAuthenticator::new()?;

    let mut builder = git2::build::RepoBuilder::new();
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(authenticator.create_callbacks());
    builder.fetch_options(fetch_options);

    let repo = builder.clone(url, path.as_ref())?;
    Ok(repo)
}

/// Convenience function to fetch from a repository trying all SSH keys
pub fn fetch_with_ssh_keys(repo: &Repository) -> Result<(), Box<dyn std::error::Error>> {
    let mut authenticator = SSHKeyAuthenticator::new()?;

    let mut remote = repo.find_remote("origin")?;
    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(authenticator.create_callbacks());

    remote.fetch(&[] as &[&str], Some(&mut fetch_options), None)?;
    Ok(())
}

/// Convenience function to push to a repository trying all SSH keys
pub fn push_with_ssh_keys(
    repo: &Repository,
    refspecs: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut authenticator = SSHKeyAuthenticator::new()?;

    let mut remote = repo.find_remote("origin")?;
    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(authenticator.create_callbacks());

    remote.push(refspecs, Some(&mut push_options))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_discover_ssh_keys() {
        let temp_dir = TempDir::new().unwrap();
        let ssh_dir = temp_dir.path();

        // Create mock SSH keys
        let mut private_key = File::create(ssh_dir.join("id_rsa")).unwrap();
        writeln!(private_key, "-----BEGIN OPENSSH PRIVATE KEY-----").unwrap();
        writeln!(private_key, "fake key content").unwrap();
        writeln!(private_key, "-----END OPENSSH PRIVATE KEY-----").unwrap();

        let mut public_key = File::create(ssh_dir.join("id_rsa.pub")).unwrap();
        writeln!(public_key, "ssh-rsa AAAAB3... fake@example.com").unwrap();

        // Create another key without public key
        let mut private_key2 = File::create(ssh_dir.join("id_ed25519")).unwrap();
        writeln!(private_key2, "-----BEGIN OPENSSH PRIVATE KEY-----").unwrap();
        writeln!(private_key2, "fake ed25519 key").unwrap();
        writeln!(private_key2, "-----END OPENSSH PRIVATE KEY-----").unwrap();

        // Create non-key files that should be ignored
        File::create(ssh_dir.join("known_hosts")).unwrap();
        File::create(ssh_dir.join("config")).unwrap();

        let keys = SSHKeyAuthenticator::discover_ssh_keys_in_dir(ssh_dir).unwrap();

        assert_eq!(keys.len(), 2);

        // ed25519 should come first due to priority
        assert_eq!(keys[0].name, "id_ed25519");
        assert!(keys[0].public_key_path.is_none());

        assert_eq!(keys[1].name, "id_rsa");
        assert!(keys[1].public_key_path.is_some());
    }

    #[test]
    fn test_key_priority() {
        assert!(
            SSHKeyAuthenticator::key_priority("id_ed25519")
                < SSHKeyAuthenticator::key_priority("id_rsa")
        );
        assert!(
            SSHKeyAuthenticator::key_priority("id_rsa")
                < SSHKeyAuthenticator::key_priority("id_ecdsa")
        );
        assert!(
            SSHKeyAuthenticator::key_priority("id_custom")
                < SSHKeyAuthenticator::key_priority("custom_key")
        );
    }
}

// Example usage
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Discover all SSH keys
    let mut authenticator = SSHKeyAuthenticator::new()?;

    println!("Discovered SSH keys:");
    for key in authenticator.list_keys() {
        println!(
            "  - {} (public key: {})",
            key.name,
            key.public_key_path
                .as_ref()
                .map(|p| p.to_string_lossy())
                .unwrap_or("none".into())
        );
    }

    // Example: Clone a repository
    // let repo = clone_with_ssh_keys("git@github.com:user/repo.git", "/tmp/repo")?;
    // println!("Repository cloned successfully!");

    Ok(())
}
