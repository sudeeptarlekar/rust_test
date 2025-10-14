use std::fmt::Write;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct PullRequest {
    pub number: u64,
    pub change_id: Option<String>,
    pub url: String,
    pub author: String,
    pub branch_name: String,
    pub title: String,
    pub build_status: BuildStatus,
    pub mergeable: bool,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub enum BuildStatus {
    #[default]
    MISSING,
    SUCCESS,
    FAILURE,
    ERROR,
    PENDING,
    EXPECTED,
}

impl std::str::FromStr for BuildStatus {
    type Err = String;

    fn from_str(state: &str) -> std::result::Result<Self, Self::Err> {
        match state {
            "SUCCESS" => Ok(Self::SUCCESS),
            "ERROR" => Ok(Self::ERROR),
            "MISSING" => Ok(Self::MISSING),
            "PENDING" => Ok(Self::MISSING),
            "EXPECTED" => Ok(Self::EXPECTED),
            _ => Err(format!("`{state}` is not valid build status")),
        }
    }
}

impl PullRequest {
    pub fn is_build_success(&self) -> bool {
        matches!(self.build_status, BuildStatus::SUCCESS)
    }
}

fn report(mut err: &dyn (std::error::Error)) -> String {
    let mut s = format!("{err}");
    while let Some(src) = err.source() {
        let _ = write!(s, "\n\nCaused by: {src}");
        err = src;
    }
    s
}

pub async fn fetch_pull_requests() -> Result<Vec<PullRequest>> {
    let query = r#"{
        repository(owner: "marcmo", name: "playground") {
            pullRequests(states: MERGED, last: 100) {
                nodes {
                    number,
                    url,
                    headRefName,
                    author { login },
                    mergeable,
                    title,
                    commits(last: 1) {
                        nodes {
                            commit {
                                message,
                                statusCheckRollup { state }
                            }
                        }
                    }
                }
            }
        }
    }"#;

    let valid_contributors = vec![
        String::from("marcmo"),
        String::from("sudeeptarlekar"),
        String::from("itsmesamster"),
    ];
    let client = reqwest::Client::new();

    println!("Fetch Pull Requests with GQL query {query}");

    let request = client
        .post("https://api.github.com/graphql")
        .header("User-Agent", "Rust-GitHub-Client")
        .bearer_auth("AUTH_TOKEN")
        .json(&serde_json::json!({ "query": query }));

    println!("Sending out request: {request:?}");

    let response = request
        .send()
        .await
        .context("Failed to fetch the pull requests from upstream repository")
        .map_err(|err| {
            eprintln!("{}", report(&*err));
            err
        })?;

    if !response.status().is_success() {
        return Err(anyhow!("Request failed with status: {}", response.status()));
    }

    let text = response
        .text()
        .await
        .context("Missing response body in pull request GraphQL API")?;

    parse_response(&text, valid_contributors.as_slice())
}

fn parse_response(response: &str, valid_contributors: &[String]) -> Result<Vec<PullRequest>> {
    let json: Value = serde_json::from_str(response)
        .context("Could not parse response body from pull_requests GraphQL API using serde")?;

    let prs = json
        .get("data")
        .and_then(|data| data.get("repository"))
        .and_then(|repo| repo.get("pullRequests"))
        .and_then(|prs| prs.get("nodes"))
        .and_then(Value::as_array)
        .ok_or(anyhow!("Missing PR data from GraphQL response"))?;

    let mut pr_list: Vec<PullRequest> = vec![];

    for pr in prs {
        let author = pr
            .get("author")
            .and_then(|author| author.get("login"))
            .and_then(Value::as_str)
            .map(String::from)
            .ok_or(anyhow!("Missing author login in response"))?;

        if !valid_contributors.contains(&author) {
            println!("PR author {author} is not in the list of valid contributors");
        }
        let title = pr
            .get("title")
            .and_then(Value::as_str)
            .map(String::from)
            .ok_or(anyhow!("Missing PR title from Response"))?;

        let branch_name = pr
            .get("headRefName")
            .and_then(Value::as_str)
            .map(String::from)
            .ok_or(anyhow!("Missing branch name from response"))?;

        let mergeable = pr
            .get("mergeable")
            .and_then(Value::as_str)
            .map(|status| status == "MERGEABLE")
            .ok_or(anyhow!("Missing PR build status from response"))?;

        let number = pr
            .get("number")
            .and_then(Value::as_u64)
            .ok_or(anyhow!("Missing PR Number from response"))?;

        let url = pr
            .get("url")
            .and_then(Value::as_str)
            .map(String::from)
            .ok_or(anyhow!("Missign url from response"))?;

        let commit = pr
            .get("commits")
            .and_then(|c| c.get("nodes"))
            .and_then(Value::as_array)
            .and_then(|v| v.first())
            .and_then(|c| c.get("commit"))
            .ok_or(anyhow!("Missing commit for GitHub PR {url}"))?;

        let message = commit
            .get("message")
            .and_then(Value::as_str)
            .ok_or(anyhow!("Missing commit message for GitHub PR {url}"))?;

        let change_id = find_change_id(message);

        let build_status = commit
            .get("statusCheckRollup")
            .and_then(|status| status.get("state"))
            .and_then(Value::as_str)
            .map(|status| status.parse::<BuildStatus>())
            .and_then(Result::ok)
            .unwrap_or(BuildStatus::default());

        pr_list.push(PullRequest {
            title,
            number,
            branch_name,
            change_id,
            author,
            url,
            build_status,
            mergeable,
        })
    }

    Ok(pr_list)
}

fn find_change_id(message: &str) -> Option<String> {
    if let Some(change_id_index) = message.find("Change-Id: ") {
        let start = change_id_index + 11;
        let end = start + 41;

        message.get(start..end).map(String::from)
    } else {
        None
    }
}
