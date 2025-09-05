use crate::util::{format_bytes, render_progress_bar};
use reqwest::multipart::Part;
use reqwest::{Client, ClientBuilder, cookie::Jar, multipart};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ApiInfo {
    path: String,
    #[allow(dead_code)]
    min_version: u32, // Not used anywhere, so it's OK to use dead_code here
    max_version: u32,
}

#[derive(Debug)]
pub struct SynologyClient {
    base_url: String,
    http: Client,
    sid: Option<String>,
    apis: HashMap<String, ApiInfo>,
}

#[derive(Debug, Deserialize)]
pub struct AuthData {
    sid: String,
}

// Handle error codes
#[derive(Debug, Deserialize)]
pub struct SynologyError {
    code: u32,
}

impl SynologyError {
    fn into_auth_error(self) -> AuthError {
        AuthError::from_code(self.code)
    }
}
// Handle error codes

// Authentication errors
#[derive(Debug)]
pub enum AuthError {
    NoSuchAccount,     // 400
    AccountDisabled,   // 401
    PermissionDenied,  // 402
    TwoStepRequired,   // 403
    TwoStepCodeFailed, // 404
    Other(u32),        // Any other error code, not in the documentation
    ParseError,        // JSON parsing or network error
}

impl AuthError {
    pub fn from_code(code: u32) -> Self {
        match code {
            400 => AuthError::NoSuchAccount,
            401 => AuthError::AccountDisabled,
            402 => AuthError::PermissionDenied,
            403 => AuthError::TwoStepRequired,
            404 => AuthError::TwoStepCodeFailed,
            other => AuthError::Other(other),
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            AuthError::NoSuchAccount => "No such account or incorrect password",
            AuthError::AccountDisabled => "Account disabled",
            AuthError::PermissionDenied => "Permission denied",
            AuthError::TwoStepRequired => "2-step verification code required",
            AuthError::TwoStepCodeFailed => "Failed to authenticate 2-step verification code",
            AuthError::Other(_) => "Unknown authentication error",
            AuthError::ParseError => "Failed to parse authentication response",
        }
    }
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::Other(code) => write!(f, "{} (code {})", self.description(), code),
            _ => write!(f, "{}", self.description()),
        }
    }
}

impl std::error::Error for AuthError {}
// Authentication errors

// Task errors
#[derive(Debug)]
pub enum TaskError {
    FileUploadFailed,        // 400
    MaxNumberOfTasksReached, // 401
    DestinationDenied,       // 402
    DestinationDoesNotExist, // 403
    InvalidTaskId,           // 404
    InvalidTaskAction,       // 405
    NoDefaultDestination,    // 406
    SetDestinationFailed,    // 407
    FileDoesNotExist,        // 408
    Other(u32),              // Any other error code, not in documentation
    ParseError,              // JSON parsing error or network error
    // Custom errors
    EmptyFile,      // 700
    EmptyFilePath,  // 701
    NotTorrentFile, // 702
}

impl TaskError {
    pub fn from_code(code: u32) -> Self {
        match code {
            400 => TaskError::FileUploadFailed,
            401 => TaskError::MaxNumberOfTasksReached,
            402 => TaskError::DestinationDenied,
            403 => TaskError::DestinationDoesNotExist,
            404 => TaskError::InvalidTaskId,
            405 => TaskError::InvalidTaskAction,
            406 => TaskError::NoDefaultDestination,
            407 => TaskError::SetDestinationFailed,
            408 => TaskError::FileDoesNotExist,
            // Custom Error codes
            700 => TaskError::EmptyFile,
            701 => TaskError::EmptyFilePath,
            702 => TaskError::NotTorrentFile,
            other => TaskError::Other(other),
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            TaskError::FileUploadFailed => "File upload failed",
            TaskError::MaxNumberOfTasksReached => "Maximum number of tasks reached",
            TaskError::DestinationDenied => "Destination denied",
            TaskError::DestinationDoesNotExist => "Destination does not exist",
            TaskError::InvalidTaskId => "Invalid task ID",
            TaskError::InvalidTaskAction => "Invalid task action",
            TaskError::NoDefaultDestination => "No default destination configured",
            TaskError::SetDestinationFailed => "Setting destination failed",
            TaskError::FileDoesNotExist => "File does not exist",
            TaskError::ParseError => "Failed to parse response",
            TaskError::EmptyFile => "File is empty",
            TaskError::EmptyFilePath => "No file path found",
            TaskError::NotTorrentFile => "Not a .torrent file",
            TaskError::Other(_) => "Unknown error",
        }
    }
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::Other(code) => write!(f, "{} (code {})", self.description(), code),
            _ => write!(f, "{}", self.description()),
        }
    }
}

impl std::error::Error for TaskError {}
// Task errors

#[derive(Debug, Deserialize)]
pub struct ConfigData {
    pub bt_max_download: u64,
    pub bt_max_upload: u64,
    pub default_destination: Option<String>,
    pub emule_default_destination: Option<String>,
    pub emule_enabled: bool,
    pub emule_max_download: u64,
    pub emule_max_upload: u64,
    pub ftp_max_download: u64,
    pub http_max_download: u64,
    pub nzb_max_download: u64,
    pub unzip_service_enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct AuthResponse {
    success: bool,
    data: Option<AuthData>,
    error: Option<SynologyError>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigResponse {
    success: bool,
    data: ConfigData,
    #[allow(dead_code)]
    error: Option<SynologyError>,
}

#[derive(Debug, Deserialize)]
pub struct TaskListResponse {
    success: bool,
    data: Option<TaskData>,
    error: Option<SynologyError>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TaskData {
    offset: usize, // Not used anywhere so it's OK to use dead_code here
    total: usize,  // Not used anywhere so it's OK to use dead_code here
    tasks: Vec<DownloadTask>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DownloadTask {
    pub id: String,
    pub size: u64,
    pub status: TaskStatus,
    pub title: String,
    #[serde(rename = "type")]
    pub task_type: String,
    pub username: String,
    pub additional: Option<AdditionalInfo>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct AdditionalInfo {
    pub detail: Option<DetailInfo>,
    pub transfer: Option<TransferInfo>,
    pub file: Option<Vec<FileInfo>>,
    pub peer: Option<Vec<PeerInfo>>,
    pub tracker: Option<Vec<TrackerInfo>>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DetailInfo {
    pub connected_leechers: Option<u64>,
    pub connected_seeders: Option<u64>,
    pub connected_peers: Option<u64>,
    pub create_time: Option<u64>,
    pub started_time: Option<u64>,
    pub completed_time: Option<u64>,
    pub seedelapsed: Option<u64>,
    pub destination: Option<String>,
    pub priority: Option<String>,
    pub total_peers: Option<u64>,
    pub total_pieces: Option<u64>,
    pub unzip_password: Option<String>,
    pub waiting_seconds: Option<u64>,
    pub uri: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TransferInfo {
    pub size_downloaded: Option<u64>,
    pub size_uploaded: Option<u64>,
    pub downloaded_pieces: Option<u64>,
    pub speed_download: Option<u64>,
    pub speed_upload: Option<u64>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct FileInfo {
    pub filename: Option<String>,
    pub priority: Option<String>,
    pub size: Option<u64>,
    pub size_downloaded: Option<u64>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct TrackerInfo {
    pub url: Option<String>,
    pub status: Option<String>,
    pub update_timer: Option<u64>,
    pub seeds: Option<i64>,
    pub peers: Option<i64>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct PeerInfo {
    pub address: Option<String>,
    pub agent: Option<String>,
    pub progress: Option<f64>,
    pub speed_download: Option<u64>,
    pub speed_upload: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub filename: String,
    pub priority: String,
    pub size: u64,
    pub size_downloaded: u64,
}

#[derive(Debug, Clone)]
pub struct PeerEntry {
    pub address: String,
    pub agent: String,
    pub progress: f64,
    pub speed_download: u64,
    pub speed_upload: u64,
}

#[derive(Debug, Clone)]
pub struct TrackerEntry {
    pub url: String,
    pub status: String,
    pub update_timer: u64,
    pub seeds: i64,
    pub peers: i64,
}

#[derive(Debug)]
// Wanted to have a cleared up, per task struct to display the properties easier. Don't know if
// this is the good approach, but worksforme
pub struct ExtendedDownloadTask {
    pub task_id: String,
    pub task_size: u64,
    pub task_status: TaskStatus,
    pub task_title: String,
    pub task_type: String,
    pub task_ratio: f64,
    pub task_username: String,
    pub connected_leechers: u64,
    pub connected_seeders: u64,
    pub connected_peers: u64,
    pub task_create_time: u64,
    pub task_started_time: u64,
    pub task_completed_time: u64,
    pub task_seedelapsed: u64,
    pub task_destination: String,
    pub task_priority: String,
    pub total_peers: u64,
    pub total_pieces: u64,
    pub unzip_password: String,
    pub waiting_seconds: u64,
    pub task_uri: String,
    pub task_size_downloaded: u64,
    pub task_size_uploaded: u64,
    pub task_downloaded_pieces: u64,
    pub task_speed_download: u64,
    pub task_speed_upload: u64,
    pub files: Vec<FileEntry>,
    pub peers: Vec<PeerEntry>,
    pub trackers: Vec<TrackerEntry>,
}

impl From<DownloadTask> for ExtendedDownloadTask {
    fn from(task: DownloadTask) -> Self {
        let ratio = task.upload_download_ratio().unwrap_or_default();
        let additional = task.additional.unwrap_or_default();
        let detail = additional.detail.unwrap_or_default();
        let transfer = additional.transfer.unwrap_or_default();
        let files_detailed = additional
            .file
            .unwrap_or_default()
            .into_iter()
            .map(|f| FileEntry {
                filename: f.filename.unwrap_or_default(),
                priority: f.priority.unwrap_or_default(),
                size: f.size.unwrap_or(0),
                size_downloaded: f.size_downloaded.unwrap_or(0),
            })
            .collect();
        let peers_detailed = additional
            .peer
            .unwrap_or_default()
            .into_iter()
            .map(|p| PeerEntry {
                address: p.address.unwrap_or_default(),
                agent: p.agent.unwrap_or_default(),
                progress: p.progress.unwrap_or(0.0),
                speed_download: p.speed_download.unwrap_or(0),
                speed_upload: p.speed_upload.unwrap_or(0),
            })
            .collect();
        let trackers_detailed = additional
            .tracker
            .unwrap_or_default()
            .into_iter()
            .map(|t| TrackerEntry {
                url: t.url.unwrap_or_default(),
                status: t.status.unwrap_or_default(),
                update_timer: t.update_timer.unwrap_or(0),
                seeds: t.seeds.unwrap_or(0),
                peers: t.peers.unwrap_or(0),
            })
            .collect();

        Self {
            task_id: task.id,
            task_size: task.size,
            task_status: task.status,
            task_title: task.title,
            task_type: task.task_type,
            task_username: task.username,
            task_ratio: ratio,
            connected_leechers: detail.connected_leechers.unwrap_or(0),
            connected_seeders: detail.connected_seeders.unwrap_or(0),
            connected_peers: detail.connected_peers.unwrap_or(0),
            task_create_time: detail.create_time.unwrap_or(0),
            task_started_time: detail.started_time.unwrap_or(0),
            task_completed_time: detail.completed_time.unwrap_or(0),
            task_seedelapsed: detail.seedelapsed.unwrap_or(0),
            task_destination: detail.destination.unwrap_or_default(),
            task_priority: detail.priority.unwrap_or_default(),
            total_peers: detail.total_peers.unwrap_or(0),
            total_pieces: detail.total_pieces.unwrap_or(0),
            unzip_password: detail.unzip_password.unwrap_or_default(),
            waiting_seconds: detail.waiting_seconds.unwrap_or(0),
            task_uri: detail.uri.unwrap_or_default(),
            task_size_downloaded: transfer.size_downloaded.unwrap_or(0),
            task_size_uploaded: transfer.size_uploaded.unwrap_or(0),
            task_downloaded_pieces: transfer.downloaded_pieces.unwrap_or(0),
            task_speed_download: transfer.speed_download.unwrap_or(0),
            task_speed_upload: transfer.speed_upload.unwrap_or(0),
            files: files_detailed,
            peers: peers_detailed,
            trackers: trackers_detailed,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TaskDetailResponse {
    success: bool,
    pub data: Option<TaskInfoData>,
    pub error: Option<SynologyError>,
}

#[derive(Debug, Deserialize)]
pub struct TaskActionResponse {
    pub error: u32,
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct TaskActionResponseWrapper {
    pub success: bool,
    pub data: Option<Vec<TaskActionResponse>>,
    pub error: Option<SynologyError>,
}

#[derive(Debug)]
pub struct CreateTaskResponseWrapper {
    pub success: bool,
    pub error: Option<TaskError>,
}

#[derive(Debug, Deserialize)]
pub struct TaskInfoData {
    pub tasks: Vec<DownloadTask>,
}

impl SynologyClient {
    pub fn new(base_url: &str) -> Self {
        let cookie_store = Arc::new(Jar::default());
        let client = ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .cookie_provider(cookie_store)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            base_url: base_url.to_string(),
            http: client,
            sid: None,
            apis: HashMap::new(),
        }
    }

    pub async fn get_available_apis(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/webapi/query.cgi", self.base_url);
        let params = [
            ("api", "SYNO.API.Info"),
            ("version", "1"),
            ("method", "query"),
            ("query", "ALL"),
        ];

        let res = self.http.get(&url).query(&params).send().await?;
        let json: serde_json::Value = res.json().await?;

        let data = json
            .get("data")
            .ok_or("Missing `data` field")?
            .as_object()
            .ok_or("`data` is not an object")?;

        for (api_name, info) in data.iter() {
            if let (Some(path), Some(min_version), Some(max_version)) = (
                info.get("path").and_then(|v| v.as_str()),
                info.get("minVersion").and_then(|v| v.as_u64()),
                info.get("maxVersion").and_then(|v| v.as_u64()),
            ) {
                self.apis.insert(
                    api_name.to_string(),
                    ApiInfo {
                        path: path.to_string(),
                        min_version: min_version as u32,
                        max_version: max_version as u32,
                    },
                );
            }
        }

        Ok(())
    }

    pub fn api_info(&self, api: &str) -> Option<&ApiInfo> {
        self.apis.get(api)
    }

    pub fn api_url(&self, api: &str) -> Option<String> {
        self.api_info(api)
            .map(|info| format!("{}/webapi/{}", self.base_url, info.path))
    }

    // Use the max_version of the available APIs
    pub fn api_version(&self, api: &str) -> Option<u32> {
        self.api_info(api).map(|info| info.max_version)
    }

    // Handle login
    pub async fn login(
        &mut self,
        account: &str,
        password: &str,
        session: &str,
    ) -> Result<(), AuthError> {
        let url = self.api_url("SYNO.API.Auth").ok_or(AuthError::Other(0))?;
        let version = self
            .api_version("SYNO.API.Auth")
            .ok_or(AuthError::Other(0))?
            .to_string();

        let params = [
            ("api", "SYNO.API.Auth"),
            ("method", "login"),
            ("version", &version),
            ("account", account),
            ("passwd", password),
            ("session", session),
            ("format", "sid"),
        ];

        let resp = self
            .http
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|_| AuthError::ParseError)?;

        let auth: AuthResponse = resp.json().await.map_err(|_| AuthError::ParseError)?;

        if !auth.success {
            // map the numeric code to our enum
            return Err(match auth.error {
                Some(e) => e.into_auth_error(),
                None => AuthError::Other(0),
            });
        }

        // extract SID
        let sid = auth.data.ok_or(AuthError::Other(0))?.sid;
        self.sid = Some(sid);

        Ok(())
    }

    // Handle logout
    pub async fn logout(&mut self, session: &str) -> Result<(), AuthError> {
        let url = self.api_url("SYNO.API.Auth").ok_or(AuthError::Other(0))?;
        let version = self
            .api_version("SYNO.API.Auth")
            .ok_or(AuthError::Other(0))?
            .to_string();

        let params = [
            ("api", "SYNO.API.Auth"),
            ("method", "logout"),
            ("version", &version),
            ("session", session),
        ];

        let resp = self
            .http
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|_| AuthError::ParseError)?;

        let auth: AuthResponse =
            serde_json::from_str(&resp.text().await.map_err(|_| AuthError::ParseError)?)
                .map_err(|_| AuthError::ParseError)?;

        if !auth.success {
            return Err(match auth.error {
                Some(e) => e.into_auth_error(),
                None => AuthError::Other(0),
            });
        }
        println!("✅ Logged out successfully!");
        Ok(())
    }

    // Use the result of the parsed config data
    pub async fn get_config(&self) -> Result<ConfigData, Box<dyn std::error::Error>> {
        let url = self
            .api_url("SYNO.DownloadStation.Info")
            .ok_or("Missing API info for SYNO.DownloadStation.Info")?;
        let version = self
            .api_version("SYNO.DownloadStation.Info")
            .ok_or("Missing version info for SYNO.DownloadStation.Info")?
            .to_string();
        let sid = self.sid.as_ref().ok_or("Not logged in")?;
        let params = [
            ("api", "SYNO.DownloadStation.Info"),
            ("version", &version),
            ("method", "getconfig"),
            ("_sid", sid),
        ];
        let resp = self.http.get(&url).query(&params).send().await?;
        let text = resp.text().await?;
        let wrapper: ConfigResponse =
            serde_json::from_str(&text).map_err(|e| format!("parsing ConfigResponse: {e}"))?;
        if !wrapper.success {
            Err("getconfig returned success=false".into())
        } else {
            Ok(wrapper.data)
        }
    }

    pub async fn list_download_tasks(
        &self,
    ) -> Result<Vec<DownloadTask>, Box<dyn std::error::Error>> {
        let sid = self.sid.as_ref().ok_or("SID missing — not logged in")?;
        let url = self
            .api_url("SYNO.DownloadStation.Task")
            .ok_or("Missing API info for SYNO.DownloadStation.Task")?;
        let version = self
            .api_version("SYNO.DownloadStation.Task")
            .ok_or("Missing version info for SYNO.DownloadStation.Task")?
            .to_string();
        let params = [
            ("api", "SYNO.DownloadStation.Task"),
            ("method", "list"),
            ("version", &version),
            ("_sid", sid), // inject SID manually in case cookies don't work
            ("additional", "detail,transfer,file,peer,tracker"),
        ];

        let res = self.http.get(&url).query(&params).send().await?;
        let body = res.text().await?;

        let parsed: TaskListResponse = serde_json::from_str(&body)?;
        if !parsed.success {
            println!("❌ Failed to list tasks: {:?}", parsed.error);
            return Err("Failed to list tasks".into());
        }

        Ok(parsed.data.ok_or("No task data returned")?.tasks)
    }

    pub async fn list_download_task_ids(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let sid = self.sid.as_ref().ok_or("SID is missing - not logged in")?;
        let url = self
            .api_url("SYNO.DownloadStation.Task")
            .ok_or("Missing API info for SYNO.DownloadStation.Task")?;
        let version = self
            .api_version("SYNO.DownloadStation.Task")
            .ok_or("Missing API version")?
            .to_string();
        let params = [
            ("api", "SYNO.DownloadStation.Task"),
            ("method", "list"),
            ("version", &version),
            ("_sid", sid),
        ];

        let res = self.http.get(&url).query(&params).send().await?;
        let body = res.text().await?;

        let parsed: TaskListResponse = serde_json::from_str(&body)?;

        if parsed.success {
            let task_ids = parsed
                .data
                .unwrap()
                .tasks
                .into_iter()
                .map(|task| task.id)
                .collect();
            Ok(task_ids)
        } else {
            Err(format!("❌ Failed to list tasks: {:?}", parsed.error).into())
        }
    }

    pub async fn get_task_details(
        &self,
        ids: &[String],
    ) -> Result<Vec<DownloadTask>, Box<dyn std::error::Error>> {
        let sid = self.sid.as_ref().ok_or("SID missing — not logged in")?;
        let url = self
            .api_url("SYNO.DownloadStation.Task")
            .ok_or("Missing API info for SYNO.DownloadStation.Task")?;
        let version = self
            .api_version("SYNO.DownloadStation.Task")
            .ok_or("Missing API version")?
            .to_string();
        let id_list = ids.join(",");
        let params = [
            ("api", "SYNO.DownloadStation.Task"),
            ("method", "getinfo"),
            ("version", &version),
            ("_sid", sid),
            ("id", &id_list),
            ("additional", "detail,transfer,file,peer,tracker"),
        ];

        let res = self.http.get(&url).query(&params).send().await?;
        let body = res.text().await?;

        let parsed: TaskDetailResponse = serde_json::from_str(&body)?;

        if parsed.success {
            Ok(parsed.data.unwrap().tasks)
        } else {
            Err(format!("❌ Failed to fetch task info: {:?}", parsed.error).into())
        }
    }

    pub async fn pause_task(&mut self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Build URL & parameters, early‐return on missing API info or SID
        let url = self
            .api_url("SYNO.DownloadStation.Task")
            .ok_or("❌ Missing API info for SYNO.DownloadStation.Task")?;

        let version = self
            .api_version("SYNO.DownloadStation.Task")
            .ok_or("❌ Missing version info for SYNO.DownloadStation.Task")?
            .to_string();

        let sid = self
            .sid
            .as_ref()
            .ok_or("❌ No session ID (not logged in)")?;

        let params = [
            ("api", "SYNO.DownloadStation.Task"),
            ("version", &version),
            ("method", "pause"),
            ("id", id),
            ("_sid", sid),
        ];

        // Send request, map any HTTP‐level errors
        let body = self
            .http
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|e| format!("Network error during pause_task: {e}"))?
            .text()
            .await
            .map_err(|e| format!("Failed to read pause response body: {e}"))?;

        // Parse the standard WebAPI envelope
        let wrapper: TaskActionResponseWrapper = serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse pause_task envelope: {e}"))?;

        // Check overall success flag
        if !wrapper.success {
            return Err(format!("Pause API returned success=false: {:?}", wrapper.error).into());
        }

        // Unwrap the inner array of per‐task results
        let results = wrapper.data.ok_or("Pause API returned no `data` field")?;

        // Find any non-zero error codes
        if let Some(failed) = results.into_iter().find(|r| r.error != 0) {
            return Err(format!(
                "Pause failed for task {}: error code {}",
                failed.id, failed.error
            )
            .into());
        }

        // Everything OK
        Ok(())
    }

    // Exactly the same envelope‐unwrap logic as pause_task, but calling “resume”
    pub async fn resume_task(&mut self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let url = self
            .api_url("SYNO.DownloadStation.Task")
            .ok_or("Missing API info for SYNO.DownloadStation.Task")?;
        let version = self
            .api_version("SYNO.DownloadStation.Task")
            .ok_or("Missing version info for SYNO.DownloadStation.Task")?
            .to_string();

        let params = [
            ("api", "SYNO.DownloadStation.Task"),
            ("version", &version),
            ("method", "resume"), // <<< here!
            ("id", id),
            ("_sid", self.sid.as_ref().ok_or("No SID")?),
        ];

        let res = self.http.get(&url).query(&params).send().await?;
        let body = res.text().await?;

        // Parse the same envelope like in pause_task
        let wrapper: TaskActionResponseWrapper = serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse resume envelope: {e}"))?;
        if !wrapper.success {
            return Err(format!("Resume API returned success=false: {:?}", wrapper.error).into());
        }
        let responses = wrapper.data.ok_or("Resume API returned no data")?;
        let mut errs = responses
            .into_iter()
            .filter(|r| r.error != 0)
            .map(|r| (r.id, r.error))
            .collect::<Vec<_>>();
        if let Some((bad_id, code)) = errs.pop() {
            Err(format!("Resume failed for {bad_id} (code {code})").into())
        } else {
            Ok(())
        }
    }

    pub async fn delete_task(&mut self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Build URL & parameters, early‐return on missing API info or SID
        let url = self
            .api_url("SYNO.DownloadStation.Task")
            .ok_or("❌ Missing API info for SYNO.DownloadStation.Task")?;

        let version = self
            .api_version("SYNO.DownloadStation.Task")
            .ok_or("❌ Missing version info for SYNO.DownloadStation.Task")?
            .to_string();

        let sid = self
            .sid
            .as_ref()
            .ok_or("❌ No session ID (not logged in)")?;

        let params = [
            ("api", "SYNO.DownloadStation.Task"),
            ("version", &version),
            ("method", "delete"),
            ("id", id),
            ("_sid", sid),
        ];

        // Send request, map any HTTP‐level errors
        let body = self
            .http
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|e| format!("Network error during delete_task: {e}"))?
            .text()
            .await
            .map_err(|e| format!("Failed to read delete response body: {e}"))?;

        // Parse the standard WebAPI envelope
        let wrapper: TaskActionResponseWrapper = serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse delete_task envelope: {e}"))?;

        // Check overall success flag
        if !wrapper.success {
            return Err(format!("Delete API returned success=false: {:?}", wrapper.error).into());
        }

        // Unwrap the inner array of per‐task results
        let results = wrapper.data.ok_or("Delete API returned no `data` field")?;

        // Find any non-zero error codes
        if let Some(failed) = results.into_iter().find(|r| r.error != 0) {
            return Err(format!(
                "Delete failed for task {}: error code {}",
                failed.id, failed.error
            )
            .into());
        }

        // Everything OK
        Ok(())
    }

    // Add task from URL
    pub async fn create_task_from_url(&self, uri: &str) -> Result<(), TaskError> {
        // Build URL & parameters, early‐return on missing API info or SID
        let api = "SYNO.DownloadStation.Task";
        let endpoint = self.api_url(api).ok_or(TaskError::ParseError)?; // missing API info is unexpected here
        let version = self
            .api_version(api)
            .ok_or(TaskError::ParseError)?
            .to_string();

        let sid = self.sid.as_ref().ok_or(TaskError::ParseError)?;
        let params = [
            ("api", api),
            ("version", &version),
            ("method", "create"),
            ("uri", uri),
            ("_sid", sid),
        ];

        let resp = self
            .http
            .get(&endpoint)
            .query(&params)
            .send()
            .await
            .map_err(|_| TaskError::ParseError)?;
        let text = resp.text().await.map_err(|_| TaskError::ParseError)?;

        #[derive(serde::Deserialize)]
        struct RawResponse {
            success: bool,
            error: Option<ErrorObj>,
        }
        #[derive(serde::Deserialize)]
        struct ErrorObj {
            code: u32,
        }

        let parsed: RawResponse = serde_json::from_str(&text).map_err(|_| TaskError::ParseError)?;

        if parsed.success {
            Ok(())
        } else if let Some(err) = parsed.error {
            Err(TaskError::from_code(err.code))
        } else {
            Err(TaskError::ParseError)
        }
    }

    // Add task from file
    pub async fn create_task_from_file(
        &self,
        file_path: String,
        file_data: &[u8],
    ) -> Result<(), TaskError> {
        // Build URL & parameters, early‐return on missing API info or SID
        let api = "SYNO.DownloadStation2.Task";
        let version = "2";
        let sid = self.sid.as_ref().ok_or(TaskError::ParseError)?;
        let endpoint = format!("{}/webapi/entry.cgi", self.base_url);

        // Validation
        if file_data.is_empty() {
            return Err(TaskError::EmptyFile);
        }

        if file_path.is_empty() {
            return Err(TaskError::EmptyFilePath);
        }

        if !file_path.ends_with(".torrent") {
            return Err(TaskError::NotTorrentFile);
        }

        // Create multipart form
        // File part
        let file_part = Part::bytes(file_data.to_vec())
            .file_name(file_path)
            .mime_str("application/x-bittorrent")
            .unwrap();
        // TODO: what is the context here? Until then use the unwrap above
        // .context("Failed to create file part")?;

        // Form
        let form = multipart::Form::new()
            .text("api", api)
            .text("version", version)
            .text("method", "create")
            .text("type", "\"file\"")
            .text("file", "[\"torrent\"]")
            .text("create_list", "false")
            .part("torrent", file_part);

        let url = format!("{endpoint}?_sid={sid}");

        let resp = self
            .http
            .post(url)
            .multipart(form)
            .send()
            .await
            .map_err(|_| TaskError::ParseError)?;
        let text = resp.text().await.map_err(|_| TaskError::ParseError)?;

        #[derive(serde::Deserialize)]
        struct RawResponse {
            success: bool,
            error: Option<ErrorObj>,
        }
        #[derive(serde::Deserialize)]
        struct ErrorObj {
            code: u32,
        }

        let parsed: RawResponse = serde_json::from_str(&text).map_err(|_| TaskError::ParseError)?;

        if parsed.success {
            Ok(())
        } else if let Some(err) = parsed.error {
            Err(TaskError::from_code(err.code))
        } else {
            Err(TaskError::ParseError)
        }
    }
}

impl DownloadTask {
    // Calculate ratio for torrent files
    pub fn upload_download_ratio(&self) -> Option<f64> {
        let additional = self.additional.as_ref()?;
        let transfer = additional.transfer.as_ref()?;
        let downloaded = transfer.size_downloaded?;
        let uploaded = transfer.size_uploaded?;

        if downloaded == 0 {
            return None;
        }

        Some(uploaded as f64 / downloaded as f64)
    }

    // Handle all task types (at least every task type in the old documentation)
    pub fn task_type(&self) -> &str {
        match self.task_type.as_str() {
            "bt" => "Bittorrent",
            "ftp" => "FTP download",
            "http" => "HTTP download",
            "https" => "HTTPS download",
            _ => "Other type of download",
        }
    }
}

impl ExtendedDownloadTask {
    // Create a Vec from the downloaded info so the table can handle it later
    pub fn to_row_cells(&self) -> Vec<String> {
        // let mut cells = Vec::new();
        let downloaded = self.task_size_downloaded;
        let total = self.task_size;
        let pct = if total > 0 {
            ((downloaded as f64 / total as f64) * 100.0).round() as u64
        } else {
            0
        };
        let cells = vec![
            self.task_title.clone(),
            format_bytes(self.task_size),
            format_bytes(self.task_size_downloaded),
            format_bytes(self.task_size_uploaded),
            render_progress_bar(pct, 10),
            format_bytes(self.task_speed_upload),
            format_bytes(self.task_speed_download),
            format!("{:.2}", self.task_ratio),
            self.task_status.label().to_string(),
        ];
        cells
    }
}

// For some reason the API provides both string and numbers as status... Oh, well...
#[derive(Debug, Clone)]
pub enum TaskStatus {
    Code(u64),
    Name(String),
}

// The official API documentation is from 2014 and I'm sure these codes are not perfectly match
// anymore
impl TaskStatus {
    pub fn label(&self) -> &str {
        match self {
            TaskStatus::Name(s) => s, // The API sometimes sends back a text status, need
            // to handle that as well

            // All other codes I could find
            TaskStatus::Code(code) => match code {
                1 => "waiting",
                2 => "downloading",
                3 => "paused",
                4 => "finishing",
                5 => "finished",
                6 => "hash_checking",
                7 => "preseeding",
                8 => "seeding",
                9 => "filehost_waiting",
                10 => "extracting",
                11 => "preprocessing",
                12 => "preprocesspass",
                13 => "downloaded",
                14 => "postprocessing",
                15 => "captcha_needed",
                101 => "error",
                102 => "broken_link",
                103 => "dest_not_exists",
                104 => "dest_deny",
                105 => "disk_full",
                106 => "quota_reached",
                107 => "timeout",
                108 => "exceed_max_fs_size",
                109 => "exceed_max_temp_fs_size",
                110 => "exceed_max_dest_fs_size",
                111 => "name_too_long_encryption",
                112 => "name_too_long",
                113 => "duplicate_torrent",
                114 => "file_does_not_exist",
                115 => "premium_required",
                116 => "not_supported_type",
                117 => "ftp_encrypt_not_supported",
                118 => "extract_failed",
                119 => "extract_wrong_pasword",
                120 => "extract_invalid_archive",
                121 => "extract_quota_reached",
                122 => "extract_disk_full",
                123 => "invalid_torrent",
                124 => "account_required",
                125 => "try_it_later",
                126 => "encryption_error",
                127 => "missing_python_executable",
                128 => "private_video",
                129 => "extract_folder_does_not_exist",
                130 => "nzb_missing_article",
                131 => "duplicate_edonkey_link",
                132 => "duplicate_dest_file",
                133 => "archive_repair_failed",
                134 => "invalid_account_password",
                _ => "unknown",
            },
        }
    }
}

impl<'de> Deserialize<'de> for TaskStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TaskStatusVisitor;

        impl serde::de::Visitor<'_> for TaskStatusVisitor {
            type Value = TaskStatus;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a string or integer status")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(TaskStatus::Name(v.to_string()))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(TaskStatus::Code(v))
            }
        }

        deserializer.deserialize_any(TaskStatusVisitor)
    }
}
