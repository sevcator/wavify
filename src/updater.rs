use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn repository() -> Option<&'static str> {
    option_env!("WAVIFY_GITHUB_REPOSITORY")
        .filter(|value| value.split('/').count() == 2 && !value.contains(' '))
}

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub notes: String,
    installer_url: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    body: String,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

pub async fn check_latest() -> Result<Option<Release>> {
    let Some(repository) = repository() else {
        return Ok(None);
    };
    let url = format!("https://api.github.com/repos/{repository}/releases/latest");
    let response = reqwest::Client::builder()
        .user_agent(concat!("Wavify/", env!("CARGO_PKG_VERSION")))
        .build()?
        .get(url)
        .send()
        .await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let release: GithubRelease = response.error_for_status()?.json().await?;
    let version = release.tag_name.trim_start_matches('v').to_string();
    if !is_newer(&version, APP_VERSION) {
        return Ok(None);
    }
    let installer_url = release
        .assets
        .into_iter()
        .find(|asset| asset.name.eq_ignore_ascii_case("Wavify-Setup.exe"))
        .map(|asset| asset.browser_download_url)
        .with_context(|| format!("Release {} has no Wavify-Setup.exe asset", release.tag_name))?;
    Ok(Some(Release {
        version,
        notes: release.body,
        installer_url,
    }))
}

pub async fn download_installer(release: Release) -> Result<PathBuf> {
    let bytes = reqwest::Client::builder()
        .user_agent(concat!("Wavify/", env!("CARGO_PKG_VERSION")))
        .build()?
        .get(release.installer_url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if bytes.len() < 2 || &bytes[..2] != b"MZ" {
        bail!("The downloaded update is not a Windows installer");
    }
    let path = std::env::temp_dir().join(format!("wavify-update-{}.exe", release.version));
    tokio::fs::write(&path, bytes).await?;
    Ok(path)
}

fn is_newer(candidate: &str, current: &str) -> bool {
    fn parts(version: &str) -> Option<[u64; 3]> {
        let mut values = [0; 3];
        let mut parsed = version.split('.');
        for value in &mut values {
            *value = parsed.next()?.parse().ok()?;
        }
        parsed.next().is_none().then_some(values)
    }
    matches!((parts(candidate), parts(current)), (Some(next), Some(now)) if next > now)
}
