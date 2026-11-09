//! Fixed-source Web bundle preparation and immutable static HTTP response data.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fmt, fs,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::Arc,
};

/// Manifest stored beside the distributed Web entry point.
pub const WEB_BUNDLE_MANIFEST: &str = "web-bundle-manifest.json";
const MAX_MANIFEST_BYTES: u64 = 1_048_576;
const MAX_FILES: usize = 256;
const MAX_FILE_BYTES: u64 = 32 * 1_048_576;
const MAX_BUNDLE_BYTES: u64 = 128 * 1_048_576;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    source_sha: String,
    contract_sha256: String,
    files: Vec<ManifestFile>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ManifestFile {
    path: String,
    size: u64,
    sha256: String,
    content_type: String,
}

/// Error preventing an incomplete or incompatible bundle from being served.
#[derive(Debug)]
pub enum WebDistributionError {
    /// A bundle file could not be read or written.
    Io(io::Error),
    /// A manifest could not be decoded.
    Json(serde_json::Error),
    /// A manifest, path, byte count or identity violated the bundle contract.
    Invalid(&'static str),
}

impl fmt::Display for WebDistributionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "Web bundle I/O failed: {error}"),
            Self::Json(error) => write!(formatter, "Web bundle manifest is invalid: {error}"),
            Self::Invalid(reason) => write!(formatter, "Web bundle rejected: {reason}"),
        }
    }
}

impl std::error::Error for WebDistributionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Invalid(_) => None,
        }
    }
}

impl From<io::Error> for WebDistributionError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for WebDistributionError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

/// Validated response data owned independently of subsequent filesystem changes.
#[derive(Clone)]
pub struct WebAsset {
    /// Immutable response bytes.
    pub bytes: Arc<[u8]>,
    /// MIME type selected by the shared bundle policy.
    pub content_type: &'static str,
    /// Cache policy for the corresponding immutable or entry-point resource.
    pub cache_control: &'static str,
}

/// A complete Web build validated once when a host service starts.
pub struct WebDistribution {
    files: BTreeMap<String, WebAsset>,
    manifest_sha256: String,
}

impl WebDistribution {
    /// Loads a bundle whose full source and canonical runtime contract match the host.
    ///
    /// Every file is read, bounded and hashed before this value becomes available.
    /// HTTP requests then borrow validated memory rather than reopening disk paths.
    pub fn open(
        root: &Path,
        expected_source_sha: &str,
        expected_contract_sha256: &str,
    ) -> Result<Self, WebDistributionError> {
        identity(expected_source_sha, expected_contract_sha256)?;
        let root = distribution_root(root)?;
        let manifest_bytes = read_regular(&root, WEB_BUNDLE_MANIFEST, MAX_MANIFEST_BYTES)?;
        let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
        if manifest.schema_version != 1
            || manifest.source_sha != expected_source_sha
            || manifest.contract_sha256 != expected_contract_sha256
        {
            return Err(WebDistributionError::Invalid("source or contract mismatch"));
        }
        if manifest.files.is_empty() || manifest.files.len() > MAX_FILES {
            return Err(WebDistributionError::Invalid(
                "file count exceeds the bundle limit",
            ));
        }
        let mut files = BTreeMap::new();
        let mut total = 0_u64;
        for entry in manifest.files {
            let mime = content_type(&entry.path)?;
            if entry.path == WEB_BUNDLE_MANIFEST
                || entry.path.starts_with("api/")
                || entry.content_type != mime
                || !hexadecimal(&entry.sha256, 64)
                || entry.size > MAX_FILE_BYTES
            {
                return Err(WebDistributionError::Invalid("invalid file metadata"));
            }
            total = total
                .checked_add(entry.size)
                .filter(|total| *total <= MAX_BUNDLE_BYTES)
                .ok_or(WebDistributionError::Invalid("bundle byte limit exceeded"))?;
            let bytes = read_regular(&root, &entry.path, entry.size)?;
            if bytes.len() as u64 != entry.size || hash(&bytes) != entry.sha256 {
                return Err(WebDistributionError::Invalid(
                    "file bytes differ from the manifest",
                ));
            }
            let cache_control = if content_addressed_asset(&entry.path, &entry.sha256) {
                "public, max-age=31536000, immutable"
            } else {
                "no-store"
            };
            if files
                .insert(
                    entry.path,
                    WebAsset {
                        bytes: bytes.into(),
                        content_type: mime,
                        cache_control,
                    },
                )
                .is_some()
            {
                return Err(WebDistributionError::Invalid("duplicate manifest path"));
            }
        }
        if !files.contains_key("index.html") {
            return Err(WebDistributionError::Invalid("index.html is missing"));
        }
        let manifest_sha256 = hash(&manifest_bytes);
        files.insert(
            WEB_BUNDLE_MANIFEST.into(),
            WebAsset {
                bytes: manifest_bytes.into(),
                content_type: "application/json",
                cache_control: "no-store",
            },
        );
        Ok(Self {
            files,
            manifest_sha256,
        })
    }

    /// Hash of the exact manifest bytes advertised by the host bootstrap.
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    /// Returns only an explicitly listed raw URI path; `/` names `index.html`.
    ///
    /// The HTTP adapter passes the URI path before percent decoding and handles
    /// bootstrap and WebSocket routes separately.
    pub fn asset(&self, raw_uri_path: &str) -> Option<WebAsset> {
        let path = if raw_uri_path == "/" {
            "index.html"
        } else {
            raw_uri_path.strip_prefix('/')?
        };
        if !normal_path(path) {
            return None;
        }
        self.files.get(path).cloned()
    }
}

/// Creates a deterministic manifest for a finished relative-base Web build.
///
/// This build-time producer shares its path, MIME and capacity rules with serving.
pub fn write_web_bundle_manifest(
    root: &Path,
    source_sha: &str,
    contract_sha256: &str,
) -> Result<PathBuf, WebDistributionError> {
    identity(source_sha, contract_sha256)?;
    let root = distribution_root(root)?;
    let mut pending = vec![root.clone()];
    let mut paths = Vec::new();
    let mut entries = 0;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            entries += 1;
            if entries > MAX_FILES * 2 {
                return Err(WebDistributionError::Invalid(
                    "directory entry limit exceeded",
                ));
            }
            let entry = entry?;
            let metadata = fs::symlink_metadata(entry.path())?;
            if linked(&metadata) {
                return Err(WebDistributionError::Invalid("linked bundle entry"));
            }
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                let path = entry
                    .path()
                    .strip_prefix(&root)
                    .map_err(|_| WebDistributionError::Invalid("bundle path escaped its root"))?
                    .to_str()
                    .ok_or(WebDistributionError::Invalid("non-UTF-8 bundle path"))?
                    .replace('\\', "/");
                if path != WEB_BUNDLE_MANIFEST {
                    paths.push(path);
                }
            } else {
                return Err(WebDistributionError::Invalid("non-regular bundle entry"));
            }
        }
    }
    paths.sort();
    if paths.is_empty() || paths.len() > MAX_FILES {
        return Err(WebDistributionError::Invalid(
            "file count exceeds the bundle limit",
        ));
    }
    let mut total = 0_u64;
    let mut files = Vec::new();
    for path in paths {
        let content_type = content_type(&path)?.into();
        let bytes = read_regular(&root, &path, MAX_FILE_BYTES)?;
        total = total
            .checked_add(bytes.len() as u64)
            .filter(|total| *total <= MAX_BUNDLE_BYTES)
            .ok_or(WebDistributionError::Invalid("bundle byte limit exceeded"))?;
        files.push(ManifestFile {
            path,
            size: bytes.len() as u64,
            sha256: hash(&bytes),
            content_type,
        });
    }
    let manifest = Manifest {
        schema_version: 1,
        source_sha: source_sha.into(),
        contract_sha256: contract_sha256.into(),
        files,
    };
    let mut bytes = serde_json::to_vec_pretty(&manifest)?;
    bytes.push(b'\n');
    let path = root.join(WEB_BUNDLE_MANIFEST);
    fs::write(&path, bytes)?;
    WebDistribution::open(&root, source_sha, contract_sha256)?;
    Ok(path)
}

fn identity(source: &str, contract: &str) -> Result<(), WebDistributionError> {
    if hexadecimal(source, 40) && hexadecimal(contract, 64) {
        Ok(())
    } else {
        Err(WebDistributionError::Invalid(
            "full source and contract hashes are required",
        ))
    }
}

fn hexadecimal(value: &str, size: usize) -> bool {
    value.len() == size
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn normal_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 240
        && path.split('/').all(|part| {
            !part.is_empty()
                && !part.starts_with('.')
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        })
}

fn content_type(path: &str) -> Result<&'static str, WebDistributionError> {
    if !normal_path(path) {
        return Err(WebDistributionError::Invalid("non-canonical file path"));
    }
    match path.rsplit_once('.').map(|(_, suffix)| suffix) {
        Some("html") => Ok("text/html; charset=utf-8"),
        Some("js" | "mjs") => Ok("application/javascript"),
        Some("css") => Ok("text/css"),
        Some("wasm") => Ok("application/wasm"),
        Some("json" | "map") => Ok("application/json"),
        Some("svg") => Ok("image/svg+xml"),
        Some("png") => Ok("image/png"),
        Some("jpg" | "jpeg") => Ok("image/jpeg"),
        Some("webp") => Ok("image/webp"),
        Some("woff") => Ok("font/woff"),
        Some("woff2") => Ok("font/woff2"),
        Some("ttf") => Ok("font/ttf"),
        Some("otf") => Ok("font/otf"),
        Some("txt") => Ok("text/plain; charset=utf-8"),
        _ => Err(WebDistributionError::Invalid(
            "unsupported static file type",
        )),
    }
}

fn content_addressed_asset(path: &str, sha256: &str) -> bool {
    path.starts_with("assets/")
        && path
            .rsplit_once('.')
            .is_some_and(|(stem, _)| stem.ends_with(&format!("-{sha256}")))
}

fn distribution_root(path: &Path) -> Result<PathBuf, WebDistributionError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || linked(&metadata) {
        return Err(WebDistributionError::Invalid(
            "bundle root must be a regular directory",
        ));
    }
    Ok(path.canonicalize()?)
}

fn read_regular(root: &Path, path: &str, limit: u64) -> Result<Vec<u8>, WebDistributionError> {
    if !normal_path(path) {
        return Err(WebDistributionError::Invalid("non-canonical file path"));
    }
    let mut absolute = root.to_path_buf();
    for component in path.split('/') {
        absolute.push(component);
        if linked(&fs::symlink_metadata(&absolute)?) {
            return Err(WebDistributionError::Invalid("linked bundle entry"));
        }
    }
    if !absolute.canonicalize()?.starts_with(root) || !fs::metadata(&absolute)?.is_file() {
        return Err(WebDistributionError::Invalid(
            "bundle file escaped its root",
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(absolute)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(WebDistributionError::Invalid("file byte limit exceeded"));
    }
    Ok(bytes)
}

fn linked(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    false
}

fn hash(bytes: &[u8]) -> String {
    use fmt::Write;
    let mut output = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "birdman-web-bundle-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::create_dir(path.join("assets")).unwrap();
            fs::write(
                path.join("index.html"),
                b"<!doctype html><title>Birdman</title>",
            )
            .unwrap();
            fs::write(path.join("assets/app-original.js"), b"export {};").unwrap();
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn serving_uses_only_verified_immutable_bytes_and_explicit_paths() {
        let fixture = Fixture::new();
        let source = "a".repeat(40);
        let contract = "b".repeat(64);
        let content_hash = hash(b"export {};");
        let addressed_path = format!("assets/app-{content_hash}.js");
        fs::write(fixture.0.join(&addressed_path), b"export {};").unwrap();
        let manifest_path = write_web_bundle_manifest(&fixture.0, &source, &contract).unwrap();
        let expected_manifest = fs::read(&manifest_path).unwrap();
        let distribution = WebDistribution::open(&fixture.0, &source, &contract).unwrap();
        assert_eq!(distribution.manifest_sha256(), hash(&expected_manifest));
        assert_eq!(distribution.asset("/").unwrap().cache_control, "no-store");
        assert_eq!(
            distribution
                .asset("/assets/app-original.js")
                .unwrap()
                .cache_control,
            "no-store"
        );
        assert_eq!(
            distribution
                .asset(&format!("/{addressed_path}"))
                .unwrap()
                .cache_control,
            "public, max-age=31536000, immutable"
        );
        fs::write(fixture.0.join("assets/app-original.js"), b"changed").unwrap();
        fs::write(fixture.0.join("private.txt"), b"not distributed").unwrap();
        assert_eq!(
            &*distribution.asset("/assets/app-original.js").unwrap().bytes,
            b"export {};"
        );
        assert_eq!(
            &*distribution
                .asset("/web-bundle-manifest.json")
                .unwrap()
                .bytes,
            expected_manifest
        );
        for path in [
            "/private.txt",
            "/../index.html",
            "/%2e%2e/index.html",
            "/assets\\app-original.js",
        ] {
            assert!(distribution.asset(path).is_none());
        }
        assert!(WebDistribution::open(&fixture.0, &source, &contract).is_err());
    }

    #[test]
    fn incompatible_identity_and_traversal_manifest_are_rejected_before_serving() {
        let fixture = Fixture::new();
        let source = "a".repeat(40);
        let contract = "b".repeat(64);
        let manifest_path = write_web_bundle_manifest(&fixture.0, &source, &contract).unwrap();
        assert!(WebDistribution::open(&fixture.0, &"c".repeat(40), &contract).is_err());
        assert!(WebDistribution::open(&fixture.0, &source, &"d".repeat(64)).is_err());
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["files"][0]["path"] = "../outside.js".into();
        fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(WebDistribution::open(&fixture.0, &source, &contract).is_err());
    }
}
